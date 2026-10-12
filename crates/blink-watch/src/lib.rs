use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::Sender;
use notify_debouncer_full::notify::{Config, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, NoCache, new_debouncer_opt};

pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(75);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file watcher: {0}")]
    Notify(#[from] notify_debouncer_full::notify::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Change {
    pub root: PathBuf,
    pub worktree_paths: BTreeSet<PathBuf>,
    pub git_state_changed: bool,
}

pub struct RepoWatcher {
    _debouncer: Debouncer<RecommendedWatcher, NoCache>,
}

fn is_relevant_git_path(rel: &Path) -> bool {
    let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy());
    matches!(
        first.as_deref(),
        Some(
            "HEAD"
                | "index"
                | "MERGE_HEAD"
                | "FETCH_HEAD"
                | "ORIG_HEAD"
                | "CHERRY_PICK_HEAD"
                | "REVERT_HEAD"
                | "packed-refs"
                | "refs"
                | "rebase-merge"
                | "rebase-apply"
        )
    )
}

// FSEvents and inotify report resolved paths (macOS temp dirs live behind the /var -> /private/var symlink).
#[cfg(not(windows))]
fn real_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(windows)]
fn real_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub fn classify(root: &Path, git_dir: &Path, paths: impl IntoIterator<Item = PathBuf>) -> Change {
    let mut change = Change { root: root.to_path_buf(), ..Default::default() };
    for path in paths {
        if let Ok(rel) = path.strip_prefix(git_dir) {
            if rel.extension().is_some_and(|e| e == "lock") {
                continue;
            }
            if is_relevant_git_path(rel) {
                change.git_state_changed = true;
            }
        } else if let Ok(rel) = path.strip_prefix(root) {
            change.worktree_paths.insert(rel.to_path_buf());
        }
    }
    change
}

impl RepoWatcher {
    pub fn start(root: &Path, git_dir: &Path, debounce: Duration, tx: Sender<Change>) -> Result<Self, Error> {
        let root = real_path(root);
        let git_dir = real_path(git_dir);
        let (root, git_dir) = (root.as_path(), git_dir.as_path());
        let root_owned = root.to_path_buf();
        let git_dir_owned = git_dir.to_path_buf();
        let handler = move |result: DebounceEventResult| match result {
            Ok(events) => {
                let change = classify(&root_owned, &git_dir_owned, events.into_iter().flat_map(|e| e.event.paths));
                if change.git_state_changed || !change.worktree_paths.is_empty() {
                    let _ = tx.send(change);
                }
            }
            Err(errors) => {
                for e in errors {
                    tracing::warn!(error = %e, "watch error");
                }
            }
        };
        let mut debouncer =
            new_debouncer_opt::<_, RecommendedWatcher, NoCache>(debounce, None, handler, NoCache, Config::default())?;
        debouncer.watch(root, RecursiveMode::Recursive)?;
        if !git_dir.starts_with(root) {
            debouncer.watch(git_dir, RecursiveMode::Recursive)?;
        }
        Ok(Self { _debouncer: debouncer })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_git_and_worktree_paths() {
        let root = PathBuf::from("/r");
        let git = root.join(".git");
        let change =
            classify(&root, &git, [git.join("index.lock"), git.join("objects/ab/cd"), root.join("src/main.rs")]);
        assert!(!change.git_state_changed);
        assert_eq!(change.worktree_paths.len(), 1);

        let change = classify(&root, &git, [git.join("refs/heads/main")]);
        assert!(change.git_state_changed);
    }

    #[test]
    fn delivers_debounced_events() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        std::fs::create_dir(&git).unwrap();
        let (tx, rx) = crossbeam_channel::unbounded();
        let _w = RepoWatcher::start(dir.path(), &git, Duration::from_millis(30), tx).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let change = rx.recv_timeout(Duration::from_secs(5)).expect("event");
        assert!(change.worktree_paths.iter().any(|p| p.ends_with("a.txt")));
    }
}
