use std::sync::atomic::{AtomicU64, Ordering};

use blink_git::{Error, HeadState, Repository, Result};
use parking_lot::Mutex;
use serde::Serialize;

const MAX_ENTRIES: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UndoAction {
    MoveBranch { branch: Option<String>, sha: String },
    RestoreBranch { name: String, sha: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoEntry {
    pub id: u64,
    pub label: String,
    pub action: UndoAction,
}

#[derive(Debug, Default)]
pub struct UndoJournal {
    entries: Mutex<Vec<UndoEntry>>,
    seq: AtomicU64,
}

fn backup_ref(id: u64) -> String {
    format!("refs/blink/undo/{id}")
}

pub fn head_position(repo: &Repository) -> Result<Option<(Option<String>, String)>> {
    Ok(match repo.head()? {
        HeadState::Branch { name, id } => Some((Some(name), id)),
        HeadState::Detached { id } => Some((None, id)),
        HeadState::Unborn { .. } => None,
    })
}

impl UndoJournal {
    pub fn entries(&self) -> Vec<UndoEntry> {
        self.entries.lock().iter().rev().cloned().collect()
    }

    pub fn last(&self) -> Option<UndoEntry> {
        self.entries.lock().last().cloned()
    }

    fn push(&self, repo: &Repository, label: String, action: UndoAction) -> Result<()> {
        let id = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let sha = match &action {
            UndoAction::MoveBranch { sha, .. } | UndoAction::RestoreBranch { sha, .. } => sha.clone(),
        };
        repo.cli().update_ref(&backup_ref(id), &sha)?;
        let mut entries = self.entries.lock();
        entries.push(UndoEntry { id, label, action });
        if entries.len() > MAX_ENTRIES {
            let dropped = entries.remove(0);
            let _ = repo.cli().delete_ref(&backup_ref(dropped.id));
        }
        Ok(())
    }

    pub fn record<R>(&self, repo: &Repository, label: &str, op: impl FnOnce() -> Result<R>) -> Result<R> {
        let before = head_position(repo)?;
        let result = op();
        let after = head_position(repo).ok().flatten();
        if let Some((branch, sha)) = before
            && after.as_ref().map(|(_, s)| s) != Some(&sha)
        {
            self.push(repo, label.to_string(), UndoAction::MoveBranch { branch, sha })?;
        }
        result
    }

    pub fn record_branch_delete(&self, repo: &Repository, name: &str, sha: &str) -> Result<()> {
        self.push(
            repo,
            format!("Delete branch {name}"),
            UndoAction::RestoreBranch { name: name.into(), sha: sha.into() },
        )
    }

    pub fn undo(&self, repo: &Repository) -> Result<UndoEntry> {
        let entry = self.last().ok_or_else(|| Error::Git("nothing to undo".into()))?;
        let cli = repo.cli();
        match &entry.action {
            UndoAction::MoveBranch { branch, sha } => {
                let current = head_position(repo)?.and_then(|(b, _)| b);
                if branch.is_some() && current != *branch {
                    return Err(Error::Git(format!(
                        "switch to {} to undo \"{}\"",
                        branch.as_deref().unwrap_or("HEAD"),
                        entry.label
                    )));
                }
                if repo.is_operation_in_progress().is_some() {
                    return Err(Error::Git("finish or abort the operation in progress first".into()));
                }
                cli.reset_keep(sha)?;
            }
            UndoAction::RestoreBranch { name, sha } => cli.branch_at(name, sha, false)?,
        }
        self.entries.lock().pop();
        let _ = cli.delete_ref(&backup_ref(entry.id));
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blink_git::{GitCli, ResetMode};

    fn fixture() -> (tempfile::TempDir, Repository) {
        let dir = tempfile::tempdir().unwrap();
        GitCli::init(dir.path(), "main").unwrap();
        let cli = GitCli::new(dir.path());
        for (k, v) in [("user.name", "T"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")] {
            cli.run(&["config", k, v]).unwrap();
        }
        for i in 0..3 {
            std::fs::write(dir.path().join("f"), format!("{i}\n")).unwrap();
            cli.stage_all().unwrap();
            cli.commit(&format!("c{i}"), false).unwrap();
        }
        let repo = Repository::open(dir.path()).unwrap();
        (dir, repo)
    }

    #[test]
    fn undo_hard_reset_restores_commits() {
        let (_dir, repo) = fixture();
        let journal = UndoJournal::default();
        journal.record(&repo, "Reset", || repo.cli().reset("HEAD~2", ResetMode::Hard)).unwrap();
        assert_eq!(repo.log(None, 0, 10).unwrap().len(), 1);
        let entry = journal.undo(&repo).unwrap();
        assert_eq!(entry.label, "Reset");
        assert_eq!(repo.log(None, 0, 10).unwrap().len(), 3);
        assert!(journal.last().is_none());
    }

    #[test]
    fn no_entry_when_head_unchanged() {
        let (_dir, repo) = fixture();
        let journal = UndoJournal::default();
        let _ = journal.record(&repo, "noop", || repo.cli().run(&["status"]));
        assert!(journal.entries().is_empty());
    }

    #[test]
    fn restores_deleted_branch() {
        let (_dir, repo) = fixture();
        let cli = repo.cli();
        cli.create_branch("gone", None, false).unwrap();
        let sha = cli.rev_parse("gone").unwrap();
        let journal = UndoJournal::default();
        journal.record_branch_delete(&repo, "gone", &sha).unwrap();
        cli.delete_branch("gone", true).unwrap();
        journal.undo(&repo).unwrap();
        assert_eq!(cli.rev_parse("gone").unwrap(), sha);
    }
}
