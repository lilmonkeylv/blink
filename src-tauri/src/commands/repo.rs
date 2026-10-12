use std::path::PathBuf;

use blink_git::{GitCli, Repository};
use blink_github::Account;
use blink_store::{RepoRecord, Settings};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;
use crate::view::RepoView;
use crate::watch::{ensure_watcher, stop_watcher};

const LAST_REPO_KEY: &str = "lastRepository";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    repositories: Vec<RepoRecord>,
    settings: Settings,
    account: Option<Account>,
    last_repository: Option<String>,
    oauth_configured: bool,
    git_found: bool,
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> CmdResult<Bootstrap> {
    Ok(Bootstrap {
        repositories: state.cache.repositories()?,
        settings: state.settings.read().clone(),
        account: state.account(),
        last_repository: state.cache.app_state(LAST_REPO_KEY)?,
        oauth_configured: state.oauth.client_id.is_some(),
        git_found: blink_git::GitCli::new(".").run(&["--version"]).is_ok(),
    })
}

fn owner_from_remote(url: &str) -> Option<String> {
    let path = if let Some(rest) = url.strip_prefix("git@") {
        rest.split_once(':')?.1
    } else {
        let parsed = url::Url::parse(url).ok()?;
        return parsed.path_segments()?.next().map(str::to_string).filter(|s| !s.is_empty());
    };
    path.split('/').next().map(str::to_string)
}

pub(crate) fn origin_url(repo: &Repository) -> Option<String> {
    let remotes = repo.cli().remotes().ok()?;
    remotes.iter().find(|(name, _)| name == "origin").or(remotes.first()).map(|(_, url)| url.clone())
}

fn record_for(repo: &Repository) -> RepoRecord {
    let remote_url = origin_url(repo);
    RepoRecord {
        path: repo.workdir().to_string_lossy().into_owned(),
        name: repo.name(),
        owner: remote_url.as_deref().and_then(owner_from_remote),
        remote_url,
        last_opened: 0,
    }
}

#[tauri::command]
pub async fn add_repository(state: State<'_, AppState>, path: String) -> CmdResult<RepoRecord> {
    let repo = tauri::async_runtime::spawn_blocking(move || Repository::open(PathBuf::from(path)))
        .await
        .map_err(|e| CommandError::new("core", e.to_string()))??;
    let record = record_for(&repo);
    state.cache.add_repository(&record)?;
    Ok(record)
}

#[tauri::command]
pub async fn init_repository(state: State<'_, AppState>, path: String) -> CmdResult<RepoRecord> {
    let branch = state.settings.read().git.default_branch.clone();
    GitCli::init(&PathBuf::from(&path), &branch)?;
    add_repository(state, path).await
}

const MIT: &str = "MIT License

Copyright (c) [year] [fullname]

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the \"Software\"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
";

fn current_year() -> u64 {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    1970 + secs / 31_556_952
}

#[tauri::command]
pub async fn create_repository(
    state: State<'_, AppState>,
    parent: String,
    name: String,
    description: Option<String>,
    readme: bool,
    gitignore: Option<String>,
    license: Option<String>,
) -> CmdResult<RepoRecord> {
    let dir = PathBuf::from(&parent).join(&name);
    if dir.join(".git").exists() {
        return Err(CommandError::new("exists", format!("{} is already a repository", dir.display())));
    }
    let io = |e: std::io::Error| CommandError::new("io", e.to_string());
    let gh = state.github();
    let gitignore_body = match (gitignore.filter(|g| !g.is_empty()), &gh) {
        (Some(template), Some(gh)) => Some(gh.gitignore_template(&template).await?),
        _ => None,
    };
    let license_body = match license.filter(|l| !l.is_empty()) {
        Some(key) if key == "mit" => Some(MIT.to_string()),
        Some(key) => match &gh {
            Some(gh) => Some(gh.license_text(&key).await?),
            None => None,
        },
        None => None,
    };

    let branch = state.settings.read().git.default_branch.clone();
    GitCli::init(&dir, &branch)?;
    let mut wrote = false;
    if readme {
        let mut body = format!(
            "# {name}
"
        );
        if let Some(d) = description.filter(|d| !d.trim().is_empty()) {
            body.push_str(&format!(
                "
{}
",
                d.trim()
            ));
        }
        std::fs::write(dir.join("README.md"), body).map_err(io)?;
        wrote = true;
    }
    if let Some(body) = gitignore_body {
        std::fs::write(dir.join(".gitignore"), body).map_err(io)?;
        wrote = true;
    }
    if let Some(body) = license_body {
        let holder = state.account().and_then(|a| a.name.or(Some(a.login))).unwrap_or_default();
        let text = body.replace("[year]", &current_year().to_string()).replace("[fullname]", &holder);
        std::fs::write(dir.join("LICENSE"), text).map_err(io)?;
        wrote = true;
    }
    if wrote {
        let cli = GitCli::new(&dir);
        cli.stage_all()?;
        cli.commit("Initial commit", false)?;
    }
    add_repository(state, dir.to_string_lossy().into_owned()).await
}

#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSettings {
    remotes: Vec<(String, String)>,
    gitignore: String,
    user_name: Option<String>,
    user_email: Option<String>,
}

#[tauri::command]
pub async fn repo_settings(state: State<'_, AppState>, id: String) -> CmdResult<RepoSettings> {
    Ok(state
        .engine
        .read(&id, |repo| {
            let cli = repo.cli();
            Ok(RepoSettings {
                remotes: cli.remotes()?,
                gitignore: std::fs::read_to_string(repo.workdir().join(".gitignore")).unwrap_or_default(),
                user_name: cli.local_config("user.name"),
                user_email: cli.local_config("user.email"),
            })
        })
        .await?)
}

#[tauri::command]
pub async fn save_repo_settings(state: State<'_, AppState>, id: String, settings: RepoSettings) -> CmdResult<()> {
    Ok(state
        .engine
        .write(&id, move |repo| {
            let cli = repo.cli();
            let existing = cli.remotes()?;
            for (name, url) in &settings.remotes {
                match existing.iter().find(|(n, _)| n == name) {
                    Some((_, current)) if current == url => {}
                    Some(_) => cli.set_remote_url(name, url)?,
                    None if !url.trim().is_empty() => cli.add_remote(name, url.trim())?,
                    None => {}
                }
            }
            for (name, _) in existing.iter().filter(|(n, _)| !settings.remotes.iter().any(|(m, _)| m == n)) {
                cli.remove_remote(name)?;
            }
            let path = repo.workdir().join(".gitignore");
            let current = std::fs::read_to_string(&path).unwrap_or_default();
            if current != settings.gitignore && !(current.is_empty() && settings.gitignore.trim().is_empty()) {
                std::fs::write(&path, &settings.gitignore)?;
            }
            cli.set_local_config("user.name", settings.user_name.as_deref())?;
            cli.set_local_config("user.email", settings.user_email.as_deref())
        })
        .await?)
}

fn scan_for_repositories(root: &std::path::Path, depth: usize, out: &mut Vec<PathBuf>) {
    if root.join(".git").exists() {
        out.push(root.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let hidden = path.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.'));
        let skip = path.file_name().is_some_and(|n| matches!(n.to_string_lossy().as_ref(), "node_modules" | "target"));
        if path.is_dir() && !hidden && !skip {
            scan_for_repositories(&path, depth - 1, out);
        }
    }
}

#[tauri::command]
pub async fn import_repositories(state: State<'_, AppState>, root: String) -> CmdResult<Vec<RepoRecord>> {
    let found = tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        scan_for_repositories(std::path::Path::new(&root), 4, &mut out);
        out
    })
    .await
    .map_err(|e| CommandError::new("core", e.to_string()))?;
    let mut added = Vec::new();
    for path in found {
        if let Ok(repo) = Repository::open(&path) {
            let record = record_for(&repo);
            state.cache.add_repository(&record)?;
            added.push(record);
        }
    }
    Ok(added)
}

#[tauri::command]
pub fn remove_repository(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    stop_watcher(&state, &path);
    state.engine.close(&path);
    state.cache.remove_repository(&path)?;
    Ok(())
}

#[tauri::command]
pub async fn open_repository(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<RepoView> {
    let (id, handle) = state.engine.open(&path).await?;
    let snapshot = state.engine.refresh(&id).await?;
    ensure_watcher(&app, &state, &id)?;
    let _ = state.cache.touch_repository(&id);
    let _ = state.cache.put_app_state(LAST_REPO_KEY, &id);
    Ok(RepoView::from_snapshot(&id, handle.repo().name(), &snapshot))
}

#[tauri::command]
pub async fn refresh_repository(state: State<'_, AppState>, id: String) -> CmdResult<RepoView> {
    let handle = state.engine.get(&id)?;
    let snapshot = match state.engine.refresh(&id).await {
        Ok(s) => s,
        Err(blink_core::Error::Cancelled) => handle.snapshot(),
        Err(e) => return Err(e.into()),
    };
    Ok(RepoView::from_snapshot(&id, handle.repo().name(), &snapshot))
}

#[tauri::command]
pub fn reveal_in_file_manager(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    path: Option<String>,
) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let target = match path {
        Some(p) => handle.repo().workdir().join(p),
        None => handle.repo().workdir().to_path_buf(),
    };
    app.opener().reveal_item_in_dir(target).map_err(|e| CommandError::new("opener", e.to_string()))
}

pub(crate) fn web_url(remote: &str) -> Option<String> {
    let trimmed = remote.trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("https://{host}/{path}"));
    }
    let mut url = url::Url::parse(trimmed).ok()?;
    if url.scheme() == "ssh" {
        let host = url.host_str()?.to_string();
        return Some(format!("https://{host}{}", url.path()));
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    Some(url.to_string())
}

#[tauri::command]
pub fn open_on_github(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let url = origin_url(handle.repo())
        .and_then(|u| web_url(&u))
        .ok_or_else(|| CommandError::new("noRemote", "This repository has no remote"))?;
    app.opener().open_url(url, None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[tauri::command]
pub async fn remotes(state: State<'_, AppState>, id: String) -> CmdResult<Vec<(String, String)>> {
    Ok(state.engine.read(&id, |repo| repo.cli().remotes()).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owners_and_web_urls() {
        assert_eq!(owner_from_remote("https://github.com/acme/app.git").as_deref(), Some("acme"));
        assert_eq!(owner_from_remote("git@github.com:acme/app.git").as_deref(), Some("acme"));
        assert_eq!(web_url("git@github.com:acme/app.git").as_deref(), Some("https://github.com/acme/app"));
        assert_eq!(web_url("https://user@github.com/acme/app.git").as_deref(), Some("https://github.com/acme/app"));
        assert_eq!(web_url("ssh://git@github.com/acme/app.git").as_deref(), Some("https://github.com/acme/app"));
    }
}
