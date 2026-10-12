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

impl RepoView {
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
