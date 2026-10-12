use std::time::Duration;

use blink_git::{HeadState, Progress, SyncAction};
use blink_store::PullStrategy;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;
use crate::view::RepoView;
use crate::watch::REPO_CHANGED;

pub const SYNC_PROGRESS: &str = "sync-progress";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    repo: String,
    action: SyncAction,
    progress: Progress,
}

pub(crate) fn progress_emitter(
    app: &AppHandle,
    repo: &str,
    action: SyncAction,
) -> impl FnMut(Progress) + Send + 'static {
    let app = app.clone();
    let repo = repo.to_string();
    move |progress| {
        let _ = app.emit(SYNC_PROGRESS, ProgressEvent { repo: repo.clone(), action, progress });
    }
}

#[tauri::command]
pub async fn sync(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    action: SyncAction,
    force_with_lease: Option<bool>,
    tags: Option<bool>,
) -> CmdResult<()> {
    let handle = state.engine.get(&id)?;
    let cli = state.git_cli(handle.repo());
    let (prune, pull_strategy) = {
        let settings = state.settings.read();
        (settings.git.prune_on_fetch, settings.git.pull_strategy)
    };
    let force = force_with_lease.unwrap_or(false);
    let push_tags = tags.unwrap_or(false);
    let mut emit = progress_emitter(&app, &id, action);

    let label = match action {
        SyncAction::Pull => "Pull",
        _ => "Sync",
    };
    state
        .engine
        .rewrite(&id, label, move |repo| match action {
            SyncAction::Fetch => cli.fetch(None, prune, emit),
            SyncAction::Pull => match pull_strategy {
                PullStrategy::FfOnly => cli.run(&["pull", "--ff-only"]).map(drop),
                strategy => cli.pull(strategy == PullStrategy::Rebase, emit),
            },
            SyncAction::Push => {
                let HeadState::Branch { name, .. } = repo.head()? else {
                    return Err(blink_git::Error::Git("Cannot push a detached HEAD".into()));
                };
                let upstream = repo.branches()?.into_iter().find(|b| b.name == name).and_then(|b| b.upstream);
                let remote = upstream
                    .as_deref()
                    .and_then(|u| u.split_once('/'))
                    .map(|(r, _)| r.to_string())
                    .or_else(|| cli.remotes().ok()?.first().map(|(n, _)| n.clone()))
                    .ok_or_else(|| blink_git::Error::Git("This repository has no remote to push to".into()))?;
                cli.push(&remote, &name, upstream.is_none(), force, &mut emit)?;
                if push_tags {
                    cli.push_tags(&remote, &mut emit)?;
                }
                Ok(())
            }
        })
        .await
        .map_err(CommandError::from)
}

pub fn spawn_background_fetch(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_run = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let state = app.state::<AppState>();
            let (enabled, minutes, prune) = {
                let s = state.settings.read();
                (s.advanced.background_fetch, s.advanced.background_fetch_minutes.max(5), s.git.prune_on_fetch)
            };
            if !enabled || last_run.elapsed() < Duration::from_secs(minutes as u64 * 60) {
                continue;
            }
            last_run = std::time::Instant::now();
            let engine = state.engine.clone();
            for id in engine.open_repos() {
                let Ok(handle) = engine.get(&id) else { continue };
                let cli = state.git_cli(handle.repo());
                let fetched = engine.network_write(&id, move |_| cli.fetch(None, prune, |_| {})).await;
                if let Err(e) = fetched {
                    tracing::debug!(repo = %id, error = %e, "background fetch failed");
                    continue;
                }
                if let Ok(handle) = engine.get(&id) {
                    let view = RepoView::from_snapshot(&id, handle.repo().name(), &handle.snapshot());
                    let _ = app.emit(REPO_CHANGED, view);
                }
            }
        }
    });
}
