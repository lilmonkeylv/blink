use std::path::PathBuf;
use std::time::Duration;

use blink_git::{GitCli, HeadState, Repository, SyncAction};
use blink_github::{CheckState, ChecksSummary, Client, Org, PullRequest, Repo, parse_remote};
use blink_store::RepoRecord;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

use crate::commands::repo::{add_repository, origin_url};
use crate::commands::sync::progress_emitter;
use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

pub const CHECKS_CHANGED: &str = "checks-changed";

fn client(state: &AppState) -> CmdResult<Client> {
    state.github().ok_or_else(|| CommandError::new("signedOut", "Sign in to GitHub.com first"))
}

fn remote_url(repo: &Repository, name: &str) -> Option<String> {
    repo.cli().remotes().ok()?.into_iter().find(|(n, _)| n == name).map(|(_, u)| u)
}

pub fn github_coords(repo: &Repository, remote: Option<&str>) -> Option<(String, String)> {
    let url = match remote {
        Some(name) => remote_url(repo, name)?,
        None => origin_url(repo)?,
    };
    let (host, owner, name) = parse_remote(&url)?;
    (host == "github.com").then_some((owner, name))
}

fn coords(repo: &Repository) -> CmdResult<(String, String)> {
    github_coords(repo, None)
        .ok_or_else(|| CommandError::new("notGithub", "This repository is not hosted on GitHub.com"))
}

#[tauri::command]
pub async fn github_repositories(state: State<'_, AppState>) -> CmdResult<Vec<Repo>> {
    Ok(client(&state)?.repos().await?)
}

#[tauri::command]
pub async fn github_orgs(state: State<'_, AppState>) -> CmdResult<Vec<Org>> {
    Ok(client(&state)?.orgs().await?)
}

#[tauri::command]
pub async fn gitignore_templates(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(client(&state)?.gitignore_templates().await?)
}

#[tauri::command]
pub async fn clone_repository(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    parent: String,
    name: String,
    depth: Option<u32>,
) -> CmdResult<RepoRecord> {
    let dest = PathBuf::from(&parent).join(&name);
    if dest.exists() && std::fs::read_dir(&dest).map(|mut d| d.next().is_some()).unwrap_or(false) {
        return Err(CommandError::new("exists", format!("{} already exists and is not empty", dest.display())));
    }
    let template = state.authenticate(GitCli::new(&parent), Some(&url));
    let emit = progress_emitter(&app, "clone", SyncAction::Fetch);
    let permit =
        state.engine.network().clone().acquire_owned().await.map_err(|e| CommandError::new("core", e.to_string()))?;
    let clone_dest = dest.clone();
    tauri::async_runtime::spawn_blocking(move || GitCli::clone_repository(&url, &clone_dest, depth, &template, emit))
        .await
        .map_err(|e| CommandError::new("core", e.to_string()))??;
    drop(permit);
    add_repository(state, dest.to_string_lossy().into_owned()).await
}

#[tauri::command]
pub async fn publish_repository(
    state: State<'_, AppState>,
    id: String,
    name: String,
    description: String,
    private: bool,
    org: Option<String>,
) -> CmdResult<Repo> {
    let gh = client(&state)?;
    let created = gh.create_repo(org.as_deref(), &name, &description, private).await?;
    let handle = state.engine.get(&id)?;
    let cli = state.authenticate(handle.repo().cli(), Some(&created.clone_url));
    let url = created.clone_url.clone();
    state
        .engine
        .network_write(&id, move |repo| {
            match remote_url(repo, "origin") {
                Some(_) => cli.set_remote_url("origin", &url)?,
                None => cli.add_remote("origin", &url)?,
            }
            if let HeadState::Branch { name, .. } = repo.head()? {
                cli.push("origin", &name, true, false, |_| {})?;
            }
            Ok(())
        })
        .await?;
    let _ = state.cache.add_repository(&RepoRecord {
        path: id.clone(),
        name: handle.repo().name(),
        owner: Some(created.owner.login.clone()),
        remote_url: Some(created.clone_url.clone()),
        last_opened: 0,
    });
    Ok(created)
}

#[tauri::command]
pub async fn pull_requests(state: State<'_, AppState>, id: String) -> CmdResult<Vec<PullRequest>> {
    let handle = state.engine.get(&id)?;
    let repo = handle.repo();
    let (owner, name) = github_coords(repo, Some("upstream")).map(Ok).unwrap_or_else(|| coords(repo))?;
    Ok(client(&state)?.pulls(&owner, &name).await?)
}

#[tauri::command]
pub async fn checkout_pull_request(
    state: State<'_, AppState>,
    id: String,
    number: u64,
    head_ref: String,
) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let remote = if remote_url(handle.repo(), "upstream").is_some() { "upstream" } else { "origin" };
    let cli = state.git_cli(handle.repo());
    let remote = remote.to_string();
    Ok(state
        .engine
        .network_write(&id, move |repo| {
            let taken = repo.branches()?.iter().any(|b| b.name == head_ref);
            let local = if taken { format!("pr/{number}") } else { head_ref.clone() };
            let current = repo.head()?.branch_name().map(str::to_string);
            if current.as_deref() == Some(local.as_str()) {
                return cli.run(&["pull", "--ff-only", &remote, &format!("pull/{number}/head")]).map(drop);
            }
            cli.fetch_pull_request(&remote, number, &local)?;
            cli.checkout(&local)
        })
        .await?)
}

#[tauri::command]
pub async fn create_pull_request(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let repo = handle.repo();
    let (owner, name) = coords(repo)?;
    let branch = repo
        .head()?
        .branch_name()
        .map(str::to_string)
        .ok_or_else(|| CommandError::new("detached", "Check out a branch first"))?;
    let url = match github_coords(repo, Some("upstream")) {
        Some((up_owner, up_name)) => {
            format!("https://github.com/{up_owner}/{up_name}/compare/{owner}:{branch}?expand=1")
        }
        None => format!("https://github.com/{owner}/{name}/compare/{branch}?expand=1"),
    };
    app.opener().open_url(url, None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[tauri::command]
pub async fn branch_checks(state: State<'_, AppState>, id: String) -> CmdResult<Option<ChecksSummary>> {
    let handle = state.engine.get(&id)?;
    let Some((owner, name)) = github_coords(handle.repo(), None) else { return Ok(None) };
    let Some(HeadState::Branch { id: sha, .. }) = handle.snapshot().head.clone() else { return Ok(None) };
    let Some(gh) = state.github() else { return Ok(None) };
    Ok(Some(gh.checks(&owner, &name, &sha).await?))
}

#[tauri::command]
pub async fn rerun_failed_checks(state: State<'_, AppState>, id: String, suites: Vec<u64>) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let (owner, name) = coords(handle.repo())?;
    let gh = client(&state)?;
    for suite in suites {
        gh.rerun_check_suite(&owner, &name, suite).await?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkInfo {
    is_fork: bool,
    parent: Option<String>,
    parent_url: Option<String>,
    has_upstream: bool,
}

#[tauri::command]
pub async fn fork_info(state: State<'_, AppState>, id: String) -> CmdResult<ForkInfo> {
    let handle = state.engine.get(&id)?;
    let has_upstream = remote_url(handle.repo(), "upstream").is_some();
    let (owner, name) = coords(handle.repo())?;
    let repo = client(&state)?.repo(&owner, &name).await?;
    Ok(ForkInfo {
        is_fork: repo.fork,
        parent: repo.parent.as_ref().map(|p| p.full_name.clone()),
        parent_url: repo.parent.as_ref().map(|p| p.clone_url.clone()),
        has_upstream,
    })
}

#[tauri::command]
pub async fn add_upstream(state: State<'_, AppState>, id: String, url: String) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let cli = state.authenticate(handle.repo().cli(), Some(&url));
    Ok(state
        .engine
        .network_write(&id, move |_| {
            cli.add_remote("upstream", &url)?;
            cli.fetch(Some("upstream"), false, |_| {})
        })
        .await?)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChecksEvent {
    repo: String,
    summary: ChecksSummary,
}

pub fn spawn_checks_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(90)).await;
            let state = app.state::<AppState>();
            let Some(gh) = state.github() else { continue };
            for id in state.engine.open_repos() {
                let Ok(handle) = state.engine.get(&id) else { continue };
                let Some((owner, name)) = github_coords(handle.repo(), None) else { continue };
                let Some(HeadState::Branch { id: sha, name: branch }) = handle.snapshot().head.clone() else {
                    continue;
                };
                let summary = match gh.checks(&owner, &name, &sha).await {
                    Ok(s) => s,
                    Err(blink_github::Error::Unauthorized) => break,
                    Err(e) => {
                        tracing::debug!(repo = %id, error = %e, "checks poll failed");
                        continue;
                    }
                };
                let key = format!("{id}@{sha}");
                let previous = state.check_states.lock().insert(key, summary.state);
                let notify = {
                    let n = &state.settings.read().notifications;
                    n.enabled && n.checks_failed
                };
                if notify && summary.state == CheckState::Failure && previous.is_some_and(|p| p != CheckState::Failure)
                {
                    let _ = app
                        .notification()
                        .builder()
                        .title(format!("Checks failed on {branch}"))
                        .body(format!("{} of {} checks failed in {owner}/{name}", summary.failed, summary.total))
                        .show();
                }
                let _ = app.emit(CHECKS_CHANGED, ChecksEvent { repo: id.clone(), summary });
            }
        }
    });
}
