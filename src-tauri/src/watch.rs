use std::sync::Arc;

use blink_watch::{DEFAULT_DEBOUNCE, RepoWatcher};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::CmdResult;
use crate::state::AppState;
use crate::view::RepoView;

pub const REPO_CHANGED: &str = "repo-changed";

pub fn ensure_watcher(app: &AppHandle, state: &AppState, id: &str) -> CmdResult<()> {
    let mut watchers = state.watchers.lock();
    if watchers.contains_key(id) {
        return Ok(());
    }
    let handle = state.engine.get(id)?;
    let repo = handle.repo();
    let (tx, rx) = crossbeam_channel::unbounded();
    let watcher = RepoWatcher::start(repo.workdir(), repo.git_dir(), DEFAULT_DEBOUNCE, tx)?;
    watchers.insert(id.to_string(), watcher);

    let app = app.clone();
    let id = id.to_string();
    std::thread::Builder::new()
        .name(format!("blink-watch-{}", repo.name()))
        .spawn(move || {
            while rx.recv().is_ok() {
                while rx.try_recv().is_ok() {}
                let (app, id) = (&app, &id);
                tauri::async_runtime::block_on(async move {
                    let state = app.state::<AppState>();
                    let engine = Arc::clone(&state.engine);
                    match engine.refresh(id).await {
                        Ok(snapshot) => {
                            let name = engine.get(id).map(|h| h.repo().name()).unwrap_or_default();
                            let _ = app.emit(REPO_CHANGED, RepoView::from_snapshot(id, name, &snapshot));
                        }
                        Err(blink_core::Error::Cancelled) => {}
                        Err(blink_core::Error::UnknownRepo(_)) => {}
                        Err(e) => tracing::warn!(repo = %id, error = %e, "background refresh failed"),
                    }
                });
            }
        })
        .map_err(|e| crate::error::CommandError::new("watch", e.to_string()))?;
    Ok(())
}

pub fn stop_watcher(state: &AppState, id: &str) {
    state.watchers.lock().remove(id);
}
