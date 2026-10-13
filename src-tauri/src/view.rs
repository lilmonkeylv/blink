use blink_core::RepoSnapshot;
use blink_git::{AheadBehind, ChangeKind, FileChange, HeadState};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileView {
    #[serde(flatten)]
    pub change: FileChange,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoView {
    pub id: String,
    pub name: String,
    pub generation: u64,
    pub head: Option<HeadState>,
    pub files: Vec<FileView>,
    pub conflicted: usize,
    pub ahead_behind: Option<AheadBehind>,
    pub operation: Option<&'static str>,
    pub has_remote: bool,
}

const CACHED_VIEW_KEY: &str = "view";
const CACHED_VIEW_MAX_FILES: usize = 5_000;

impl RepoView {
    pub fn remember(&self, cache: &blink_store::Cache) {
        if self.files.len() > CACHED_VIEW_MAX_FILES {
            return;
        }
        if let Err(e) = cache.put_repo_state(&self.id, CACHED_VIEW_KEY, self) {
            tracing::debug!(repo = %self.id, error = %e, "could not cache repository view");
        }
    }

    pub fn cached(cache: &blink_store::Cache, id: &str) -> Option<serde_json::Value> {
        cache.repo_state(id, CACHED_VIEW_KEY).ok().flatten()
    }

    pub fn from_snapshot(id: &str, name: String, snapshot: &RepoSnapshot) -> Self {
        Self {
            id: id.to_string(),
            name,
            generation: snapshot.generation,
            head: snapshot.head.clone(),
            files: snapshot.status.files.iter().map(|f| FileView { kind: f.kind(), change: f.clone() }).collect(),
            conflicted: snapshot.status.conflicted,
            ahead_behind: snapshot.ahead_behind,
            operation: snapshot.operation,
            has_remote: snapshot.has_remote,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(id: &str, files: usize) -> RepoView {
        let change = |i: usize| FileView {
            change: FileChange { path: format!("f{i}"), old_path: None, staged: None, unstaged: None },
            kind: ChangeKind::Modified,
        };
        RepoView {
            id: id.into(),
            name: "r".into(),
            generation: 7,
            head: None,
            files: (0..files).map(change).collect(),
            conflicted: 0,
            ahead_behind: None,
            operation: None,
            has_remote: true,
        }
    }

    #[test]
    fn remembers_views_but_not_huge_ones() {
        let cache = blink_store::Cache::in_memory().unwrap();
        view("/small", 2).remember(&cache);
        view("/huge", CACHED_VIEW_MAX_FILES + 1).remember(&cache);
        let small = RepoView::cached(&cache, "/small").unwrap();
        assert_eq!(small["files"].as_array().unwrap().len(), 2);
        assert_eq!(small["hasRemote"], true);
        assert!(RepoView::cached(&cache, "/huge").is_none());
    }
}
