use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use gix::bstr::BString;
use gix::status::index_worktree::Item as WorktreeItem;
use gix::status::index_worktree::iter::Summary;
use serde::Serialize;

use crate::error::{Result, ResultExt};
use crate::repo::Repository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
    TypeChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub staged: Option<ChangeKind>,
    pub unstaged: Option<ChangeKind>,
}

impl FileChange {
    pub fn kind(&self) -> ChangeKind {
        match (self.staged, self.unstaged) {
            (_, Some(ChangeKind::Conflicted)) | (Some(ChangeKind::Conflicted), _) => ChangeKind::Conflicted,
            (Some(ChangeKind::Added), Some(ChangeKind::Deleted)) => ChangeKind::Added,
            (Some(ChangeKind::Added), _) => ChangeKind::Added,
            (_, Some(k)) | (Some(k), None) => k,
            (None, None) => ChangeKind::Modified,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub files: Vec<FileChange>,
    pub conflicted: usize,
    pub tracked: usize,
}

fn worktree_kind(item: &WorktreeItem) -> Option<ChangeKind> {
    Some(match item.summary()? {
        Summary::Added => ChangeKind::Untracked,
        Summary::Removed => ChangeKind::Deleted,
        Summary::Modified => ChangeKind::Modified,
        Summary::TypeChange => ChangeKind::TypeChange,
        Summary::Renamed => ChangeKind::Renamed,
        Summary::Copied => ChangeKind::Copied,
        Summary::IntentToAdd => ChangeKind::Added,
        Summary::Conflict => ChangeKind::Conflicted,
    })
}

impl Repository {
    pub fn status(&self, interrupt: Option<Arc<AtomicBool>>) -> Result<RepoStatus> {
        let repo = self.local();
        let mut platform = repo
            .status(gix::progress::Discard)
            .git()?
            .untracked_files(gix::status::UntrackedFiles::Files)
            .index_worktree_rewrites(None);
        if let Some(flag) = interrupt {
            platform = platform.should_interrupt_owned(flag);
        }

        let mut files: BTreeMap<String, FileChange> = BTreeMap::new();

        for item in platform.into_iter(Vec::<BString>::new()).git()? {
            match item.git()? {
                gix::status::Item::IndexWorktree(item) => {
                    let Some(kind) = worktree_kind(&item) else { continue };
                    let path = item.rela_path().to_string();
                    let change = upsert(&mut files, path);
                    change.unstaged = Some(kind);
                    if let WorktreeItem::Rewrite { source, .. } = &item {
                        change.old_path = Some(source.rela_path().to_string());
                    }
                }
                gix::status::Item::TreeIndex(change) => {
                    use gix::diff::index::ChangeRef;
                    let (kind, old) = match &change {
                        ChangeRef::Addition { .. } => (ChangeKind::Added, None),
                        ChangeRef::Deletion { .. } => (ChangeKind::Deleted, None),
                        ChangeRef::Modification { .. } => (ChangeKind::Modified, None),
                        ChangeRef::Rewrite { source_location, copy, .. } => {
                            let kind = if *copy { ChangeKind::Copied } else { ChangeKind::Renamed };
                            (kind, Some(source_location.to_string()))
                        }
                    };
                    let entry = upsert(&mut files, change.location().to_string());
                    entry.staged = Some(kind);
                    if old.is_some() {
                        entry.old_path = old;
                    }
                }
            }
        }

        let files: Vec<FileChange> = files.into_values().collect();
        let conflicted = files.iter().filter(|f| f.kind() == ChangeKind::Conflicted).count();
        let tracked = repo.index_or_empty().map(|index| index.entries().len()).unwrap_or_default();
        Ok(RepoStatus { files, conflicted, tracked })
    }
}

fn upsert(files: &mut BTreeMap<String, FileChange>, path: String) -> &mut FileChange {
    files.entry(path.clone()).or_insert_with(|| FileChange { path, old_path: None, staged: None, unstaged: None })
}
