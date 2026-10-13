use std::path::PathBuf;
use std::time::Duration;

use parking_lot::Mutex;

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
use crate::state::{AppState, GITHUB_HOST};

pub const CHECKS_CHANGED: &str = "checks-changed";

fn client(state: &AppState) -> CmdResult<Client> {
    state.github().ok_or_else(|| CommandError::new("signedOut", "Sign in to GitHub.com first"))
}

fn remote_url(repo: &Repository, name: &str) -> Option<String> {
    repo.cli().remotes().ok()?.into_iter().find(|(n, _)| n == name).map(|(_, u)| u)
}

pub struct HostedRepo {
    pub host: String,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitWarnings {
    protected_branch: bool,
    unlinked_email: Option<String>,
    login: Option<String>,
}

struct LinkedEmails {
    host: String,
    login: String,
    id: u64,
    emails: Vec<String>,
}

static LINKED_EMAILS: Mutex<Vec<LinkedEmails>> = Mutex::new(Vec::new());

fn email_is_linked(email: &str, linked: &LinkedEmails) -> bool {
    let email = email.trim().to_ascii_lowercase();
    if linked.emails.iter().any(|e| e.eq_ignore_ascii_case(&email)) {
        return true;
    }
    let Some((local, domain)) = email.split_once('@') else { return false };
    let login = linked.login.to_ascii_lowercase();
    domain == format!("users.noreply.{}", linked.host) && (local == login || local == format!("{}+{login}", linked.id))
}

async fn load_linked_emails(host: &str, gh: &Client) {
    if LINKED_EMAILS.lock().iter().any(|l| l.host == host) {
        return;
    }
    let (Ok(user), Ok(emails)) = (gh.user().await, gh.emails().await) else { return };
    let emails = emails.into_iter().filter(|e| e.verified).map(|e| e.email).collect();
    LINKED_EMAILS.lock().push(LinkedEmails { host: host.to_string(), login: user.login, id: user.id, emails });
}

pub fn forget_linked_emails(host: &str) {
    LINKED_EMAILS.lock().retain(|l| l.host != host);
}

#[tauri::command]
pub async fn commit_warnings(state: State<'_, AppState>, id: String) -> CmdResult<CommitWarnings> {
    let handle = state.engine.get(&id)?;
    let Some(HostedRepo { host, owner, name }) = hosted_repo(&state, handle.repo(), None) else {
        return Ok(CommitWarnings::default());
    };
    let Some(gh) = state.client_for_host(&host) else { return Ok(CommitWarnings::default()) };
    let protected_branch = match handle.snapshot().head.clone() {
        Some(HeadState::Branch { name: branch, .. }) => {
            gh.branch_protected(&owner, &name, &branch).await.unwrap_or(false)
        }
        _ => false,
    };
    load_linked_emails(&host, &gh).await;
    let email = handle.repo().cli().config_get("user.email");
    let linked = LINKED_EMAILS.lock();
    let Some(account) = linked.iter().find(|l| l.host == host) else {
        return Ok(CommitWarnings { protected_branch, ..Default::default() });
    };
    Ok(CommitWarnings {
        protected_branch,
        unlinked_email: email.filter(|e| !email_is_linked(e, account)),
        login: Some(account.login.clone()),
    })
}

pub fn hosted_repo(state: &AppState, repo: &Repository, remote: Option<&str>) -> Option<HostedRepo> {
    let url = match remote {
        Some(name) => remote_url(repo, name)?,
        None => origin_url(repo)?,
    };
    let (host, owner, name) = parse_remote(&url)?;
    let host = host.to_ascii_lowercase();
    let known = host == GITHUB_HOST || state.enterprise_accounts().iter().any(|a| a.host == host);
    known.then_some(HostedRepo { host, owner, name })
}

fn hosted(state: &AppState, repo: &Repository) -> CmdResult<HostedRepo> {
    hosted_repo(state, repo, None).ok_or_else(|| {
        CommandError::new(
            "notGithub",
            "This repository is not hosted on GitHub or a GitHub Enterprise account you added",
        )
    })
}

fn host_client(state: &AppState, host: &str) -> CmdResult<Client> {
    state.client_for_host(host).ok_or_else(|| CommandError::new("signedOut", format!("Sign in to {host} first")))
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
    let target = hosted_repo(&state, repo, Some("upstream")).map(Ok).unwrap_or_else(|| hosted(&state, repo))?;
    Ok(host_client(&state, &target.host)?.pulls(&target.owner, &target.name).await?)
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
    let HostedRepo { host, owner, name } = hosted(&state, repo)?;
    let branch = repo
        .head()?
        .branch_name()
        .map(str::to_string)
        .ok_or_else(|| CommandError::new("detached", "Check out a branch first"))?;
    let url = match hosted_repo(&state, repo, Some("upstream")) {
        Some(up) => format!("https://{}/{}/{}/compare/{owner}:{branch}?expand=1", up.host, up.owner, up.name),
        None => format!("https://{host}/{owner}/{name}/compare/{branch}?expand=1"),
    };
    app.opener().open_url(url, None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[tauri::command]
pub async fn branch_checks(state: State<'_, AppState>, id: String) -> CmdResult<Option<ChecksSummary>> {
    let handle = state.engine.get(&id)?;
    let Some(target) = hosted_repo(&state, handle.repo(), None) else { return Ok(None) };
    let Some(HeadState::Branch { id: sha, .. }) = handle.snapshot().head.clone() else { return Ok(None) };
    let Some(gh) = state.client_for_host(&target.host) else { return Ok(None) };
    Ok(Some(gh.checks(&target.owner, &target.name, &sha).await?))
}

#[tauri::command]
pub async fn rerun_failed_checks(state: State<'_, AppState>, id: String, suites: Vec<u64>) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let HostedRepo { host, owner, name } = hosted(&state, handle.repo())?;
    let gh = host_client(&state, &host)?;
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
    let HostedRepo { host, owner, name } = hosted(&state, handle.repo())?;
    let repo = host_client(&state, &host)?.repo(&owner, &name).await?;
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
            for id in state.engine.open_repos() {
                let Ok(handle) = state.engine.get(&id) else { continue };
                let Some(HostedRepo { host, owner, name }) = hosted_repo(&state, handle.repo(), None) else { continue };
                let Some(gh) = state.client_for_host(&host) else { continue };
                let Some(HeadState::Branch { id: sha, name: branch }) = handle.snapshot().head.clone() else {
                    continue;
                };
                let summary = match gh.checks(&owner, &name, &sha).await {
                    Ok(s) => s,
                    Err(blink_github::Error::Unauthorized) => continue,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noreply_and_verified_emails_count_as_linked() {
        let linked = LinkedEmails {
            host: "github.com".into(),
            login: "Octo".into(),
            id: 42,
            emails: vec!["octo@example.com".into()],
        };
        assert!(email_is_linked("Octo@Example.com", &linked));
        assert!(email_is_linked("42+octo@users.noreply.github.com", &linked));
        assert!(email_is_linked("octo@users.noreply.github.com", &linked));
        assert!(!email_is_linked("43+octo@users.noreply.github.com", &linked));
        assert!(!email_is_linked("someone@else.com", &linked));
    }
}
