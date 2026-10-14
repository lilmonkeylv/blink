use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use blink_git::Repository;
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
    pub rescan: bool,
}

impl Change {
    pub fn needs_refresh(&self) -> bool {
        self.rescan || self.git_state_changed || !self.worktree_paths.is_empty()
    }
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

pub fn classify(root: &Path, git_dir: &Path, common_dir: &Path, paths: impl IntoIterator<Item = PathBuf>) -> Change {
    let mut change = Change { root: root.to_path_buf(), ..Default::default() };
    for path in paths {
        if let Ok(rel) = path.strip_prefix(git_dir).or_else(|_| path.strip_prefix(common_dir)) {
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
    pub fn start(repo: &Repository, debounce: Duration, tx: Sender<Change>) -> Result<Self, Error> {
        let root = real_path(repo.workdir());
        let git_dir = real_path(repo.git_dir());
        let common_dir = real_path(repo.common_dir());
        let mut watched: Vec<PathBuf> = Vec::new();
        for dir in [&root, &common_dir, &git_dir] {
            if !watched.iter().any(|w| dir.starts_with(w)) {
                watched.push(dir.clone());
            }
        }
        let repo = repo.clone();
        let handler = move |result: DebounceEventResult| {
            let change = match result {
                Ok(events) => {
                    let rescan = events.iter().any(|e| e.event.need_rescan());
                    let paths = events.into_iter().flat_map(|e| e.event.paths);
                    let mut change = classify(&root, &git_dir, &common_dir, paths);
                    repo.retain_unignored(&mut change.worktree_paths);
                    change.rescan = rescan;
                    change
                }
                Err(errors) => {
                    for e in errors {
                        tracing::warn!(error = %e, "watch error");
                    }
                    Change { root: root.clone(), rescan: true, ..Default::default() }
                }
            };
            if change.needs_refresh() {
                let _ = tx.send(change);
            }
        };
        let mut debouncer =
            new_debouncer_opt::<_, RecommendedWatcher, NoCache>(debounce, None, handler, NoCache, Config::default())?;
        for dir in &watched {
            debouncer.watch(dir, RecursiveMode::Recursive)?;
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
            classify(&root, &git, &git, [git.join("index.lock"), git.join("objects/ab/cd"), root.join("src/main.rs")]);
        assert!(!change.git_state_changed);
        assert_eq!(change.worktree_paths.len(), 1);

        let change = classify(&root, &git, &git, [git.join("refs/heads/main")]);
        assert!(change.git_state_changed);
    }

    #[test]
    fn linked_worktrees_see_refs_in_the_common_dir() {
        let root = PathBuf::from("/wt");
        let common = PathBuf::from("/main/.git");
        let git = common.join("worktrees/wt");
        let change = classify(&root, &git, &common, [common.join("refs/heads/main")]);
        assert!(change.git_state_changed);
        let change = classify(&root, &git, &common, [git.join("HEAD")]);
        assert!(change.git_state_changed);
        let change = classify(&root, &git, &common, [common.join("worktrees/other/HEAD")]);
        assert!(!change.needs_refresh());
    }

    fn fixture() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        blink_git::GitCli::init(dir.path(), "main").unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        let repo = Repository::open(dir.path()).unwrap();
        (dir, repo)
    }

    fn wait_for(rx: &crossbeam_channel::Receiver<Change>, name: &str) -> Vec<Change> {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        while std::time::Instant::now() < deadline {
            if let Ok(change) = rx.recv_timeout(Duration::from_millis(500)) {
                let done = change.worktree_paths.iter().any(|p| p.ends_with(name));
                seen.push(change);
                if done {
                    return seen;
                }
            }
        }
        panic!("no event for {name}");
    }

    #[test]
    fn delivers_debounced_events_without_ignored_paths() {
        let (dir, repo) = fixture();
        let (tx, rx) = crossbeam_channel::unbounded();
        let _w = RepoWatcher::start(&repo, Duration::from_millis(30), tx).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(dir.path().join("target").join("build.o"), "x").unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();
        let changes = wait_for(&rx, "a.txt");
        assert!(changes.iter().all(|c| c.worktree_paths.iter().all(|p| !p.starts_with("target"))));
    }
}
