use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use blink_core::RepoSnapshot;
use blink_git::{AheadBehind, ChangeKind, FileChange, HeadState};
use parking_lot::Mutex;
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
    pub bisecting: bool,
}

const CACHED_VIEW_KEY: &str = "view";
const CACHED_VIEW_MAX_FILES: usize = 5_000;

static REMEMBERED: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Default::default);
static EMITTED: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(Default::default);

impl RepoView {
    fn fingerprint(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let unversioned = Self { generation: 0, ..self.clone() };
        serde_json::to_string(&unversioned).unwrap_or_default().hash(&mut hasher);
        hasher.finish()
    }

    pub fn differs_from_last_emit(&self) -> bool {
        let fingerprint = self.fingerprint();
        EMITTED.lock().insert(self.id.clone(), fingerprint) != Some(fingerprint)
    }

    pub fn remember(&self, cache: &blink_store::Cache) {
        if self.files.len() > CACHED_VIEW_MAX_FILES {
            return;
        }
        let fingerprint = self.fingerprint();
        let mut remembered = REMEMBERED.lock();
        if remembered.get(&self.id) == Some(&fingerprint) {
            return;
        }
        match cache.put_repo_state(&self.id, CACHED_VIEW_KEY, self) {
            Ok(()) => {
                remembered.insert(self.id.clone(), fingerprint);
            }
            Err(e) => tracing::debug!(repo = %self.id, error = %e, "could not cache repository view"),
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
            bisecting: snapshot.bisecting,
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
            bisecting: false,
        }
    }

    #[test]
    fn identical_views_are_emitted_once() {
        let mut view = RepoView::from_snapshot("/emit-once", "r".into(), &blink_core::RepoSnapshot::empty());
        assert!(view.differs_from_last_emit());
        view.generation += 1;
        assert!(!view.differs_from_last_emit());
        view.has_remote = !view.has_remote;
        assert!(view.differs_from_last_emit());
    }

    #[test]
    fn remembers_views_but_not_huge_ones() {
        let cache = blink_store::Cache::in_memory().unwrap();
        view("/small", 2).remember(&cache);
        view("/huge", CACHED_VIEW_MAX_FILES + 1).remember(&cache);
        let small = RepoView::cached(&cache, "/small").unwrap();
        assert_eq!(small["files"].as_array().unwrap().len(), 2);
        assert_eq!(small["hasRemote"], true);
        assert_eq!(small["bisecting"], false);
        assert!(RepoView::cached(&cache, "/huge").is_none());
    }

    #[test]
    fn rewrites_the_cached_view_only_when_it_changes() {
        let cache = blink_store::Cache::in_memory().unwrap();
        view("/dedup", 2).remember(&cache);
        let mut next = view("/dedup", 2);
        next.generation = 8;
        next.remember(&cache);
        assert_eq!(RepoView::cached(&cache, "/dedup").unwrap()["generation"], 7);
        let mut changed = view("/dedup", 3);
        changed.generation = 9;
        changed.remember(&cache);
        let cached = RepoView::cached(&cache, "/dedup").unwrap();
        assert_eq!((cached["generation"].as_u64(), cached["files"].as_array().unwrap().len()), (Some(9), 3));
    }
}
