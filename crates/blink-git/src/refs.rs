use gix::ObjectId;
use gix::remote::Direction;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ResultExt};
use crate::repo::Repository;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HeadState {
    Branch { name: String, id: String },
    Detached { id: String },
    Unborn { name: String },
}

impl HeadState {
    pub fn branch_name(&self) -> Option<&str> {
        match self {
            HeadState::Branch { name, .. } | HeadState::Unborn { name } => Some(name),
            HeadState::Detached { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AheadBehind {
    pub ahead: usize,
    pub behind: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub name: String,
    pub full_name: String,
    pub kind: BranchKind,
    pub target: String,
    pub upstream: Option<String>,
    pub last_commit_time: i64,
}

impl Repository {
    pub fn head(&self) -> Result<HeadState> {
        let repo = self.local();
        let head = repo.head().git()?;
        let name = head.referent_name().map(|n| n.shorten().to_string());
        Ok(match (head.id(), name) {
            (Some(id), Some(name)) => HeadState::Branch { name, id: id.to_string() },
            (Some(id), None) => HeadState::Detached { id: id.to_string() },
            (None, name) => HeadState::Unborn { name: name.unwrap_or_else(|| "main".into()) },
        })
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let repo = self.local();
        let platform = repo.references().git()?;
        let mut out = Vec::new();

        for (iter, kind) in [
            (platform.local_branches().git()?, BranchKind::Local),
            (platform.remote_branches().git()?, BranchKind::Remote),
        ] {
            for reference in iter {
                let mut reference = reference.git()?;
                let full_name = reference.name().as_bstr().to_string();
                if kind == BranchKind::Remote && full_name.ends_with("/HEAD") {
                    continue;
                }
                let Ok(id) = reference.peel_to_id() else { continue };
                let last_commit_time = id
                    .object()
                    .ok()
                    .and_then(|o| o.try_into_commit().ok())
                    .and_then(|c| c.time().ok())
                    .map(|t| t.seconds)
                    .unwrap_or_default();
                let upstream = (kind == BranchKind::Local)
                    .then(|| repo.branch_remote_tracking_ref_name(reference.name(), Direction::Fetch))
                    .flatten()
                    .and_then(|r| r.ok())
                    .map(|n| n.shorten().to_string());
                out.push(Branch {
                    name: reference.name().shorten().to_string(),
                    full_name,
                    kind,
                    target: id.to_string(),
                    upstream,
                    last_commit_time,
                });
            }
        }
        Ok(out)
    }

    pub fn ahead_behind(&self, local: &str, upstream: &str) -> Result<AheadBehind> {
        let repo = self.local();
        let a = repo.rev_parse_single(local).git()?.detach();
        let b = repo.rev_parse_single(upstream).git()?.detach();
        Ok(AheadBehind { ahead: count_exclusive(&repo, a, b)?, behind: count_exclusive(&repo, b, a)? })
    }

    pub fn current_ahead_behind(&self) -> Result<Option<AheadBehind>> {
        let repo = self.local();
        let Ok(Some(head)) = repo.head_name() else { return Ok(None) };
        let Some(Ok(upstream)) = repo.branch_remote_tracking_ref_name(head.as_ref(), Direction::Fetch) else {
            return Ok(None);
        };
        if repo.try_find_reference(upstream.as_ref()).git()?.is_none() {
            return Ok(None);
        }
        self.ahead_behind(&head.as_bstr().to_string(), &upstream.as_bstr().to_string()).map(Some)
    }
}

fn count_exclusive(repo: &gix::Repository, tip: ObjectId, hidden: ObjectId) -> Result<usize> {
    if tip == hidden {
        return Ok(0);
    }
    let walk = repo.rev_walk([tip]).with_hidden([hidden]).all().git()?;
    let mut n = 0;
    for info in walk {
        info.git()?;
        n += 1;
    }
    Ok(n)
}
