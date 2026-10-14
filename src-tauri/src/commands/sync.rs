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
    let result = state
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
                let target = cli.push_target(&name)?;
                cli.push(&target, force, &mut emit)?;
                if push_tags {
                    cli.push_tags(&target.remote, &mut emit)?;
                }
                Ok(())
            }
        })
        .await
        .map_err(CommandError::from);
    if result.is_ok() && action != SyncAction::Push {
        maintain_commit_graph(&state, id);
    }
    result
}

fn maintain_commit_graph(state: &AppState, id: String) {
    let engine = state.engine.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = engine.maintain_commit_graph(&id).await {
            tracing::debug!(repo = %id, error = %e, "commit-graph maintenance skipped");
        }
    });
}

pub const FETCH_ALL_PROGRESS: &str = "fetch-all-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchAllRow {
    path: String,
    name: String,
    fetched: bool,
    error: Option<String>,
    ahead: usize,
    behind: usize,
    changes: usize,
}

async fn fetch_one(state: &AppState, path: &str, prune: bool) -> CmdResult<FetchAllRow> {
    let (id, handle) = state.engine.open(path).await?;
    let mut fetched = false;
    if handle.snapshot().has_remote {
        let cli = state.git_cli(handle.repo());
        state.engine.network_write(&id, move |_| cli.fetch(None, prune, |_| {})).await?;
        fetched = true;
    }
    let snapshot = state.engine.refresh(&id).await?;
    let (ahead, behind) = snapshot.ahead_behind.map(|ab| (ab.ahead, ab.behind)).unwrap_or_default();
    Ok(FetchAllRow {
        path: path.to_string(),
        name: handle.repo().name(),
        fetched,
        error: None,
        ahead,
        behind,
        changes: snapshot.status.files.len(),
    })
}

#[tauri::command]
pub async fn fetch_all(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<FetchAllRow>> {
    let prune = state.settings.read().git.prune_on_fetch;
    let tasks: Vec<_> = state
        .cache
        .repositories()?
        .into_iter()
        .map(|record| {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let row = fetch_one(&state, &record.path, prune).await.unwrap_or_else(|e| FetchAllRow {
                    path: record.path.clone(),
                    name: record.name.clone(),
                    fetched: false,
                    error: Some(e.detail.unwrap_or(e.message)),
                    ahead: 0,
                    behind: 0,
                    changes: 0,
                });
                let _ = app.emit(FETCH_ALL_PROGRESS, &row);
                if row.error.is_none()
                    && let Ok(handle) = state.engine.get(&row.path)
                {
                    let _ = app
                        .emit(REPO_CHANGED, RepoView::from_snapshot(&row.path, row.name.clone(), &handle.snapshot()));
                }
                row
            })
        })
        .collect();
    let mut rows = Vec::with_capacity(tasks.len());
    for task in tasks {
        if let Ok(row) = task.await {
            rows.push(row);
        }
    }
    Ok(rows)
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
                maintain_commit_graph(&state, id.clone());
                if let Ok(handle) = engine.get(&id) {
                    let view = RepoView::from_snapshot(&id, handle.repo().name(), &handle.snapshot());
                    let _ = app.emit(REPO_CHANGED, view);
                }
            }
        }
    });
}
