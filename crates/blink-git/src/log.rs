use gix::revision::walk::Sorting;
use gix::traverse::commit::simple::CommitTimeOrder;
use serde::Serialize;

use crate::error::{Result, ResultExt};
use crate::repo::Repository;
use crate::status::ChangeKind;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub time: i64,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub commit: CommitSummary,
    pub body: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: i64,
    pub files: Vec<CommitFile>,
}

fn summarize(commit: &gix::Commit<'_>) -> Result<CommitSummary> {
    let id = commit.id();
    let author = commit.author().git()?;
    let message = commit.message().git()?;
    Ok(CommitSummary {
        id: id.to_string(),
        short_id: id.to_hex_with_len(7).to_string(),
        summary: message.summary().to_string(),
        author_name: author.name.to_string(),
        author_email: author.email.to_string(),
        time: author.seconds(),
        parents: commit.parent_ids().map(|p| p.to_string()).collect(),
    })
}

impl Repository {
    pub fn log(&self, start: Option<&str>, offset: usize, limit: usize) -> Result<Vec<CommitSummary>> {
        let repo = self.local();
        let tip = match start {
            Some(spec) => repo.rev_parse_single(spec).git()?.detach(),
            None => match repo.head_id() {
                Ok(id) => id.detach(),
                Err(_) => return Ok(Vec::new()),
            },
        };
        let walk = repo.rev_walk([tip]).sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst)).all().git()?;

        let mut out = Vec::with_capacity(limit);
        for info in walk.skip(offset).take(limit) {
            let info = info.git()?;
            out.push(summarize(&info.object().git()?)?);
        }
        Ok(out)
    }

    pub fn commit_detail(&self, rev: &str) -> Result<CommitDetail> {
        let repo = self.local();
        let commit = repo.rev_parse_single(rev).git()?.object().git()?.try_into_commit().git()?;
        let summary = summarize(&commit)?;
        let committer = commit.committer().git()?;
        let message = commit.message().git()?;
        let body = message.body().map(|b| b.to_string()).unwrap_or_default();

        let parent = commit.parent_ids().next().map(|p| p.detach());
        let files = tree_changes(&repo, parent, commit.id)?;

        Ok(CommitDetail {
            commit: summary,
            body,
            committer_name: committer.name.to_string(),
            committer_email: committer.email.to_string(),
            committer_time: committer.seconds(),
            files,
        })
    }

    pub fn files_between(&self, base: &str, head: &str) -> Result<Vec<CommitFile>> {
        let repo = self.local();
        let base = repo.rev_parse_single(base).git()?.detach();
        let head = repo.rev_parse_single(head).git()?.detach();
        tree_changes(&repo, Some(base), head)
    }

    pub fn range_files(&self, oldest: &str, newest: &str) -> Result<Vec<CommitFile>> {
        let repo = self.local();
        let newest = repo.rev_parse_single(newest).git()?.detach();
        let oldest = repo.rev_parse_single(oldest).git()?.object().git()?.try_into_commit().git()?;
        let base = oldest.parent_ids().next().map(|p| p.detach());
        tree_changes(&repo, base, newest)
    }
}

fn tree_of(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::Tree<'_>> {
    repo.find_object(id).git()?.try_into_commit().git()?.tree().git()
}

fn tree_changes(repo: &gix::Repository, old: Option<gix::ObjectId>, new: gix::ObjectId) -> Result<Vec<CommitFile>> {
    let new_tree = tree_of(repo, new)?;
    let old_tree = old.map(|id| tree_of(repo, id)).transpose()?;
    let changes = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None).git()?;

    use gix::object::tree::diff::ChangeDetached as C;
    let mut files: Vec<CommitFile> = changes
        .into_iter()
        .filter_map(|change| {
            let (path, old_path, kind, mode) = match change {
                C::Addition { location, entry_mode, .. } => (location, None, ChangeKind::Added, entry_mode),
                C::Deletion { location, entry_mode, .. } => (location, None, ChangeKind::Deleted, entry_mode),
                C::Modification { location, entry_mode, .. } => (location, None, ChangeKind::Modified, entry_mode),
                C::Rewrite { location, source_location, entry_mode, copy, .. } => {
                    let kind = if copy { ChangeKind::Copied } else { ChangeKind::Renamed };
                    (location, Some(source_location.to_string()), kind, entry_mode)
                }
            };
            (!mode.is_tree()).then(|| CommitFile { path: path.to_string(), old_path, kind })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
