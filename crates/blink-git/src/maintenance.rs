use std::path::Path;
use std::time::SystemTime;

use crate::cli::GitCli;
use crate::error::Result;
use crate::repo::Repository;

fn modified(path: &Path) -> Option<SystemTime> {
    path.metadata().and_then(|m| m.modified()).ok()
}

fn newest_in(dir: &Path) -> Option<SystemTime> {
    let mut newest = modified(dir);
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let time = if entry.file_type().is_ok_and(|t| t.is_dir()) { newest_in(&path) } else { modified(&path) };
        newest = newest.max(time);
    }
    newest
}

fn newest_ref_update(common_dir: &Path) -> Option<SystemTime> {
    newest_in(&common_dir.join("refs"))
        .max(modified(&common_dir.join("packed-refs")))
        .max(newest_in(&common_dir.join("reftable")))
}

fn commit_graph_written(common_dir: &Path) -> Option<SystemTime> {
    let info = common_dir.join("objects").join("info");
    modified(&info.join("commit-graph")).max(modified(&info.join("commit-graphs").join("commit-graph-chain")))
}

fn commit_graph_stale(common_dir: &Path) -> bool {
    match commit_graph_written(common_dir) {
        None => true,
        Some(written) => newest_ref_update(common_dir).is_some_and(|refs| refs > written),
    }
}

impl Repository {
    pub fn commit_graph_is_stale(&self) -> bool {
        commit_graph_stale(self.common_dir())
    }
}

impl GitCli {
    pub fn write_commit_graph(&self) -> Result<()> {
        self.run(&["commit-graph", "write", "--reachable", "--changed-paths"]).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::time::Duration;

    use super::*;

    fn touch(path: &Path, at: SystemTime) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(path).unwrap().set_modified(at).unwrap();
    }

    fn git_dir() -> (tempfile::TempDir, SystemTime) {
        let dir = tempfile::tempdir().unwrap();
        let base = SystemTime::now();
        touch(&dir.path().join("refs/heads/main"), base);
        (dir, base)
    }

    #[test]
    fn missing_graph_is_stale() {
        let (dir, _) = git_dir();
        assert!(commit_graph_stale(dir.path()));
    }

    #[test]
    fn graph_newer_than_refs_is_fresh() {
        let (dir, base) = git_dir();
        touch(&dir.path().join("objects/info/commit-graph"), base + Duration::from_secs(60));
        assert!(!commit_graph_stale(dir.path()));
    }

    #[test]
    fn ref_update_after_graph_is_stale() {
        let (dir, base) = git_dir();
        touch(&dir.path().join("objects/info/commit-graph"), base + Duration::from_secs(60));
        touch(&dir.path().join("refs/remotes/origin/main"), base + Duration::from_secs(120));
        assert!(commit_graph_stale(dir.path()));
    }

    #[test]
    fn packed_refs_count_as_ref_updates() {
        let (dir, base) = git_dir();
        touch(&dir.path().join("objects/info/commit-graphs/commit-graph-chain"), base + Duration::from_secs(60));
        assert!(!commit_graph_stale(dir.path()));
        touch(&dir.path().join("packed-refs"), base + Duration::from_secs(120));
        assert!(commit_graph_stale(dir.path()));
    }
}
