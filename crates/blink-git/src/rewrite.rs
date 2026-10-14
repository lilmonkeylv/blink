use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, Progress};
use crate::error::{Error, Result};
use crate::validate::validate_sha;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RebaseAction {
    Pick,
    Reword,
    Squash,
    Fixup,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseStep {
    pub action: RebaseAction,
    pub sha: String,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

impl Operation {
    fn command(self) -> &'static str {
        match self {
            Operation::Merge => "merge",
            Operation::Rebase => "rebase",
            Operation::CherryPick => "cherry-pick",
            Operation::Revert => "revert",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "merge" => Operation::Merge,
            "rebase" => Operation::Rebase,
            "cherry-pick" => Operation::CherryPick,
            "revert" => Operation::Revert,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub locked: bool,
}

struct ScratchFile(tempfile::TempPath);

impl ScratchFile {
    fn new(prefix: &str, contents: &str) -> Result<Self> {
        let mut file = tempfile::Builder::new().prefix(&format!("blink-{prefix}-")).tempfile()?;
        file.write_all(contents.as_bytes())?;
        Ok(Self(file.into_temp_path()))
    }

    fn shell_path(&self) -> String {
        format!("'{}'", self.0.to_string_lossy().replace('\\', "/").replace('\'', r"'\''"))
    }
}

fn render_todo(steps: &[RebaseStep], messages: &mut Vec<ScratchFile>) -> Result<String> {
    let mut todo = String::new();
    for step in steps {
        let verb = match step.action {
            RebaseAction::Pick | RebaseAction::Reword => "pick",
            RebaseAction::Squash => "squash",
            RebaseAction::Fixup => "fixup",
            RebaseAction::Drop => "drop",
        };
        let _ = writeln!(todo, "{verb} {}", validate_sha(&step.sha)?);
        let custom = match step.action {
            RebaseAction::Reword | RebaseAction::Squash => step.message.as_deref().filter(|m| !m.trim().is_empty()),
            _ => None,
        };
        if let Some(message) = custom {
            let file = ScratchFile::new("msg", message)?;
            let _ = writeln!(todo, "exec git commit --amend --no-verify --cleanup=whitespace -F {}", file.shell_path());
            messages.push(file);
        }
    }
    Ok(todo)
}

impl GitCli {
    pub fn revert(&self, sha: &str) -> Result<()> {
        self.run(&["revert", "--no-edit", sha]).map(drop)
    }

    pub fn cherry_pick(&self, shas: &[&str]) -> Result<()> {
        let mut args = vec!["cherry-pick"];
        args.extend_from_slice(shas);
        self.run(&args).map(drop)
    }

    pub fn reset(&self, target: &str, mode: ResetMode) -> Result<()> {
        let flag = match mode {
            ResetMode::Soft => "--soft",
            ResetMode::Mixed => "--mixed",
            ResetMode::Hard => "--hard",
        };
        self.run(&["reset", "-q", flag, target]).map(drop)
    }

    pub fn reset_keep(&self, target: &str) -> Result<()> {
        self.run(&["reset", "-q", "--keep", target]).map(drop)
    }

    pub fn create_tag(&self, name: &str, target: &str, message: Option<&str>) -> Result<()> {
        match message.filter(|m| !m.trim().is_empty()) {
            Some(m) => self.run(&["tag", "-a", name, "-m", m, target]).map(drop),
            None => self.run(&["tag", name, target]).map(drop),
        }
    }

    pub fn delete_tag(&self, name: &str) -> Result<()> {
        self.run(&["tag", "-d", name]).map(drop)
    }

    pub fn tags(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .run(&["tag", "--list", "--format=%(refname:short)%00%(objectname)%00%(*objectname)"])?
            .lines()
            .filter_map(|l| {
                let mut parts = l.split('\0');
                let name = parts.next()?.to_string();
                let object = parts.next()?.to_string();
                let peeled = parts.next().filter(|p| !p.is_empty()).map(str::to_string);
                Some((name, peeled.unwrap_or(object)))
            })
            .collect())
    }

    pub fn push_tags(&self, remote: &str, progress: impl FnMut(Progress)) -> Result<()> {
        self.run_with_progress(&["push", "--progress", remote, "--tags"], progress).map(drop)
    }

    pub fn rebase_onto(&self, upstream: &str) -> Result<()> {
        self.command_rebase(&["rebase", upstream])
    }

    fn command_rebase(&self, args: &[&str]) -> Result<()> {
        self.clone().with_env("GIT_EDITOR", "true").run(args).map(drop)
    }

    pub fn rebase_interactive(&self, base: Option<&str>, steps: &[RebaseStep]) -> Result<()> {
        if steps.is_empty() {
            return Err(Error::Git("nothing to rebase".into()));
        }
        if steps.first().is_some_and(|s| matches!(s.action, RebaseAction::Squash | RebaseAction::Fixup)) {
            return Err(Error::Git("the first commit cannot be squashed into a previous one".into()));
        }
        if let Some(base) = base {
            validate_sha(base)?;
        }
        let mut messages = Vec::new();
        let todo = ScratchFile::new("todo", &render_todo(steps, &mut messages)?)?;
        let editor = format!("cp {}", todo.shell_path());
        let mut args = vec!["rebase", "-i", "--no-autosquash"];
        match base {
            Some(base) => args.push(base),
            None => args.push("--root"),
        }
        self.clone().with_env("GIT_SEQUENCE_EDITOR", editor).with_env("GIT_EDITOR", "true").run(&args).map(drop)
    }

    pub fn continue_operation(&self, op: Operation) -> Result<()> {
        let cli = self.clone().with_env("GIT_EDITOR", "true");
        match op {
            Operation::Merge => cli.run(&["commit", "--no-edit"]).map(drop),
            other => cli.run(&[other.command(), "--continue"]).map(drop),
        }
    }

    pub fn abort_operation(&self, op: Operation) -> Result<()> {
        self.run(&[op.command(), "--abort"]).map(drop)
    }

    pub fn skip_operation(&self, op: Operation) -> Result<()> {
        self.run(&[op.command(), "--skip"]).map(drop)
    }

    pub fn update_ref(&self, name: &str, sha: &str) -> Result<()> {
        self.run(&["update-ref", name, sha]).map(drop)
    }

    pub fn delete_ref(&self, name: &str) -> Result<()> {
        self.run(&["update-ref", "-d", name]).map(drop)
    }

    pub fn rev_parse(&self, rev: &str) -> Result<String> {
        Ok(self.run(&["rev-parse", "--verify", "-q", &format!("{rev}^{{commit}}")])?.trim().to_string())
    }

    pub fn branch_at(&self, name: &str, sha: &str, force: bool) -> Result<()> {
        let mut args = vec!["branch"];
        if force {
            args.push("-f");
        }
        args.extend([name, sha]);
        self.run(&args).map(drop)
    }

    pub fn clone_repository(
        url: &str,
        dest: &Path,
        depth: Option<u32>,
        partial: bool,
        template: &GitCli,
        progress: impl FnMut(Progress),
    ) -> Result<()> {
        let parent = dest.parent().ok_or_else(|| Error::Git("invalid destination".into()))?;
        std::fs::create_dir_all(parent)?;
        let dest_str = dest.to_string_lossy().into_owned();
        let depth_arg = depth.map(|d| format!("--depth={d}"));
        let mut args = vec!["clone", "--progress", "--recurse-submodules"];
        if let Some(d) = &depth_arg {
            args.push(d);
        }
        if partial {
            args.push("--filter=blob:none");
        }
        args.extend(["--", url, dest_str.as_str()]);
        template.relocated(parent).run_with_progress(&args, progress).map(drop)
    }

    pub fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        self.run(&["remote", "add", name, url]).map(drop)
    }

    pub fn set_remote_url(&self, name: &str, url: &str) -> Result<()> {
        self.run(&["remote", "set-url", name, url]).map(drop)
    }

    pub fn remove_remote(&self, name: &str) -> Result<()> {
        self.run(&["remote", "remove", name]).map(drop)
    }

    pub fn fetch_pull_request(&self, remote: &str, number: u64, local: &str) -> Result<()> {
        let refspec = format!("+refs/pull/{number}/head:refs/heads/{local}");
        self.run(&["fetch", remote, &refspec]).map(drop)
    }

    pub fn format_patch(&self, rev: &str) -> Result<String> {
        self.run(&["format-patch", "-1", "--stdout", rev])
    }

    pub fn range_patch(&self, from: &str, to: &str) -> Result<String> {
        self.run(&["diff", "--binary", from, to])
    }

    pub fn apply_patch(&self, patch: &str) -> Result<()> {
        self.run_with_stdin(&["apply", "--3way", "--whitespace=nowarn", "-"], patch.as_bytes()).map(drop)
    }

    pub fn worktrees(&self) -> Result<Vec<Worktree>> {
        let mut out = Vec::new();
        let mut current: Option<Worktree> = None;
        for line in self.run(&["worktree", "list", "--porcelain"])?.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                out.extend(current.take());
                current =
                    Some(Worktree { path: path.to_string(), head: None, branch: None, bare: false, locked: false });
            } else if let Some(wt) = current.as_mut() {
                if let Some(head) = line.strip_prefix("HEAD ") {
                    wt.head = Some(head.to_string());
                } else if let Some(branch) = line.strip_prefix("branch ") {
                    wt.branch = Some(branch.trim_start_matches("refs/heads/").to_string());
                } else if line == "bare" {
                    wt.bare = true;
                } else if line.starts_with("locked") {
                    wt.locked = true;
                }
            }
        }
        out.extend(current);
        Ok(out)
    }

    pub fn add_worktree(&self, path: &Path, branch: &str, create: bool) -> Result<()> {
        let path = path.to_string_lossy().into_owned();
        if create {
            self.run(&["worktree", "add", "-b", branch, &path]).map(drop)
        } else {
            self.run(&["worktree", "add", &path, branch]).map(drop)
        }
    }

    pub fn remove_worktree(&self, path: &str, force: bool) -> Result<()> {
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(path);
        self.run(&args).map(drop)
    }

    pub fn set_local_config(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value.filter(|v| !v.is_empty()) {
            Some(v) => self.run(&["config", "--local", key, v]).map(drop),
            None => match self.run(&["config", "--local", "--unset", key]) {
                Ok(_) => Ok(()),
                Err(Error::Cli { code: Some(5), .. }) => Ok(()),
                Err(e) => Err(e),
            },
        }
    }

    pub fn sparse_folders(&self) -> Option<Vec<String>> {
        let enabled = self.run(&["config", "--bool", "--get", "core.sparseCheckout"]).ok()?;
        if enabled.trim() != "true" {
            return None;
        }
        let list = self.run(&["sparse-checkout", "list"]).ok()?;
        Some(list.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
    }

    pub fn set_sparse_folders(&self, folders: Option<&[String]>) -> Result<()> {
        match folders {
            Some(folders) => {
                let mut args = vec!["sparse-checkout", "set", "--cone", "--"];
                args.extend(folders.iter().map(String::as_str));
                self.run(&args).map(drop)
            }
            None => self.run(&["sparse-checkout", "disable"]).map(drop),
        }
    }

    pub fn local_config(&self, key: &str) -> Option<String> {
        self.run(&["config", "--local", "--get", key]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_files_get_unique_names_and_are_removed() {
        let a = ScratchFile::new("todo", "pick abcd\n").unwrap();
        let b = ScratchFile::new("todo", "pick abcd\n").unwrap();
        assert_ne!(a.0.to_path_buf(), b.0.to_path_buf());
        assert!(a.0.file_name().unwrap().to_string_lossy().starts_with("blink-todo-"));
        assert_eq!(std::fs::read_to_string(&a.0).unwrap(), "pick abcd\n");
        let path = a.0.to_path_buf();
        drop(a);
        assert!(!path.exists());
    }

    #[test]
    fn todo_rejects_non_hex_shas() {
        let step = |sha: &str| RebaseStep { action: RebaseAction::Pick, sha: sha.into(), message: None };
        assert!(render_todo(&[step("abc1234")], &mut Vec::new()).is_ok());
        assert!(render_todo(&[step("abc1234\nexec rm -rf ~")], &mut Vec::new()).is_err());
        assert!(render_todo(&[step("HEAD")], &mut Vec::new()).is_err());
    }
}
