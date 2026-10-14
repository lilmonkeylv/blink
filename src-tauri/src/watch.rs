use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use blink_core::EngineEvent;
use blink_git::{GitCli, Repository};
use blink_store::WatcherMode;
use blink_watch::{Change, DEFAULT_DEBOUNCE, POLL_INTERVAL, RepoWatcher, WatchMode};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;

use crate::error::CmdResult;
use crate::state::AppState;
use crate::view::RepoView;

pub const REPO_CHANGED: &str = "repo-changed";

const MIN_REFRESH_INTERVAL: Duration = Duration::from_millis(300);

pub fn ensure_watcher(app: &AppHandle, state: &AppState, id: &str) -> CmdResult<()> {
    let mut watchers = state.watchers.lock();
    if watchers.contains_key(id) {
        return Ok(());
    }
    let handle = state.engine.get(id)?;
    let repo = handle.repo();
    let (tx, rx) = crossbeam_channel::unbounded();
    let watcher = start_watcher(state.settings.read().advanced.watcher, repo, tx)?;
    watchers.insert(id.to_string(), watcher);

    let app = app.clone();
    let id = id.to_string();
    std::thread::Builder::new()
        .name(format!("blink-watch-{}", repo.name()))
        .spawn(move || {
            let mut last_refresh: Option<Instant> = None;
            while rx.recv().is_ok() {
                if let Some(at) = last_refresh {
                    std::thread::sleep(MIN_REFRESH_INTERVAL.saturating_sub(at.elapsed()));
                }
                while rx.try_recv().is_ok() {}
                last_refresh = Some(Instant::now());
                let (app, id) = (&app, &id);
                tauri::async_runtime::block_on(async move {
                    let engine = Arc::clone(&app.state::<AppState>().engine);
                    match engine.refresh(id).await {
                        Ok(_) | Err(blink_core::Error::Cancelled) | Err(blink_core::Error::UnknownRepo(_)) => {}
                        Err(e) => tracing::warn!(repo = %id, error = %e, "background refresh failed"),
                    }
                });
            }
        })
        .map_err(|e| crate::error::CommandError::new("watch", e.to_string()))?;
    Ok(())
}

pub fn forward_snapshots(app: &AppHandle) {
    let app = app.clone();
    let mut events = app.state::<AppState>().engine.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(EngineEvent::SnapshotChanged { repo, generation }) => {
                    publish_snapshot(&app, &repo, Some(generation))
                }
                Ok(EngineEvent::RepoClosed { .. }) => {}
                Err(RecvError::Lagged(_)) => {
                    for repo in app.state::<AppState>().engine.open_repos() {
                        publish_snapshot(&app, &repo, None);
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

fn publish_snapshot(app: &AppHandle, id: &str, generation: Option<u64>) {
    let state = app.state::<AppState>();
    let Ok(handle) = state.engine.get(id) else { return };
    let snapshot = handle.snapshot();
    if generation.is_some_and(|g| g != snapshot.generation) {
        return;
    }
    let view = RepoView::from_snapshot(id, handle.repo().name(), &snapshot);
    view.remember(&state.cache);
    let _ = app.emit(REPO_CHANGED, view);
}

pub fn stop_watcher(state: &AppState, id: &str) {
    state.watchers.lock().remove(id);
}

pub fn restart_watchers(app: &AppHandle, state: &AppState) {
    let ids: Vec<String> = state.watchers.lock().keys().cloned().collect();
    for id in ids {
        stop_watcher(state, &id);
        if let Err(e) = ensure_watcher(app, state, &id) {
            tracing::warn!(repo = %id, error = %e.message, "could not restart the file watcher");
        }
    }
}

fn start_watcher(
    mode: WatcherMode,
    repo: &Repository,
    tx: crossbeam_channel::Sender<Change>,
) -> Result<RepoWatcher, blink_watch::Error> {
    match mode {
        WatcherMode::Polling => RepoWatcher::start_with(repo, DEFAULT_DEBOUNCE, WatchMode::Polling(POLL_INTERVAL), tx),
        // Git's built-in fsmonitor daemon only exists on Windows and macOS.
        WatcherMode::Fsmonitor if cfg!(any(windows, target_os = "macos")) => {
            let cli = repo.cli().with_config("core.fsmonitor", "true");
            let probe = move || status_fingerprint(&cli);
            RepoWatcher::start_with(repo, DEFAULT_DEBOUNCE, WatchMode::GitDirOnly, tx.clone())
                .map(|w| w.with_probe(repo.workdir(), POLL_INTERVAL, probe, tx))
        }
        _ => RepoWatcher::start(repo, DEFAULT_DEBOUNCE, tx),
    }
}

fn status_fingerprint(cli: &GitCli) -> Option<u64> {
    let status = cli.run_bytes(&["status", "--porcelain", "-z", "--untracked-files=all"]).ok()?;
    let mut hasher = std::hash::DefaultHasher::new();
    status.hash(&mut hasher);
    for entry in status.split(|b| *b == 0).filter(|e| e.len() > 3) {
        if let Ok(meta) = std::fs::metadata(cli.workdir().join(String::from_utf8_lossy(&entry[3..]).as_ref())) {
            meta.len().hash(&mut hasher);
            meta.modified().ok().hash(&mut hasher);
        }
    }
    Some(hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_tracks_edits_to_already_modified_files() {
        let dir = tempfile::tempdir().unwrap();
        GitCli::init(dir.path(), "main").unwrap();
        let cli = GitCli::new(dir.path());
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "one\n").unwrap();
        let before = status_fingerprint(&cli).unwrap();
        assert_eq!(status_fingerprint(&cli), Some(before));
        std::fs::write(&file, "one\ntwo, a longer line\n").unwrap();
        assert_ne!(status_fingerprint(&cli), Some(before));
    }
}
