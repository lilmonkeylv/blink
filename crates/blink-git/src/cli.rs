use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: String,
    pub percent: Option<u8>,
    pub line: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncAction {
    Fetch,
    Pull,
    Push,
}

#[derive(Debug, Clone)]
pub struct GitCli {
    workdir: PathBuf,
    git: PathBuf,
    env: Vec<(String, String)>,
    config: Vec<String>,
    sign_commits: bool,
}

static SYSTEM_GIT: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn find_git() -> Option<PathBuf> {
    SYSTEM_GIT
        .get_or_init(|| {
            let exe = if cfg!(windows) { "git.exe" } else { "git" };
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
                .map(|dir| dir.join(exe))
                .find(|candidate| candidate.is_file())
        })
        .clone()
}

impl GitCli {
    pub fn new(workdir: impl Into<PathBuf>) -> Self {
        Self {
            workdir: workdir.into(),
            git: find_git().unwrap_or_else(|| PathBuf::from("git")),
            env: Vec::new(),
            config: Vec::new(),
            sign_commits: false,
        }
    }

    pub fn with_executable(mut self, git: impl Into<PathBuf>) -> Self {
        self.git = git.into();
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    pub fn with_config(mut self, key: &str, value: &str) -> Self {
        self.config.push(format!("{key}={value}"));
        self
    }

    pub fn relocated(&self, workdir: impl Into<PathBuf>) -> Self {
        Self { workdir: workdir.into(), ..self.clone() }
    }

    pub fn with_signing(mut self, sign: bool) -> Self {
        self.sign_commits = sign;
        self
    }

    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    pub(crate) fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.git);
        cmd.current_dir(&self.workdir)
            .args(["-c", "core.quotepath=false", "-c", "color.ui=false"])
            .args(self.config.iter().flat_map(|c| ["-c", c.as_str()]))
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd
    }

    fn finish(args: &[&str], output: std::process::Output) -> Result<String> {
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Err(Error::Cli {
                command: args.first().copied().unwrap_or_default().to_string(),
                stderr: if stderr.is_empty() { stdout } else { stderr },
                code: output.status.code(),
            })
        }
    }

    fn spawn_error(e: std::io::Error) -> Error {
        if e.kind() == std::io::ErrorKind::NotFound { Error::GitNotFound } else { e.into() }
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        let output = self.command(args).output().map_err(Self::spawn_error)?;
        Self::finish(args, output)
    }

    pub fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = self.command(args).output().map_err(Self::spawn_error)?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        Self::finish(args, output).map(String::into_bytes)
    }

    pub fn run_with_stdin(&self, args: &[&str], input: &[u8]) -> Result<String> {
        let mut child = self.command(args).stdin(Stdio::piped()).spawn().map_err(Self::spawn_error)?;
        let mut stdin = child.stdin.take().expect("stdin piped");
        let payload = input.to_vec();
        let writer = std::thread::spawn(move || stdin.write_all(&payload));
        let output = child.wait_with_output()?;
        writer.join().map_err(|_| Error::Git("stdin writer panicked".into()))??;
        Self::finish(args, output)
    }

    pub fn run_with_progress(&self, args: &[&str], mut on_progress: impl FnMut(Progress)) -> Result<String> {
        let mut child = self.command(args).spawn().map_err(Self::spawn_error)?;
        let mut stdout = child.stdout.take().expect("stdout piped");
        let stdout_reader = std::thread::spawn(move || {
            let mut buf = Vec::new();
            stdout.read_to_end(&mut buf).map(|_| buf)
        });

        let mut stderr = child.stderr.take().expect("stderr piped");
        let mut all_stderr = Vec::new();
        let mut line = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = stderr.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            for &b in &chunk[..n] {
                all_stderr.push(b);
                if b == b'\r' || b == b'\n' {
                    if !line.is_empty() {
                        on_progress(parse_progress(&String::from_utf8_lossy(&line)));
                        line.clear();
                    }
                } else {
                    line.push(b);
                }
            }
        }
        let status = child.wait()?;
        let stdout = stdout_reader.join().map_err(|_| Error::Git("stdout reader panicked".into()))??;
        Self::finish(args, std::process::Output { status, stdout, stderr: all_stderr })
    }

    pub fn init(dir: &Path, default_branch: &str) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        GitCli::new(dir).run(&["init", "-b", default_branch]).map(drop)
    }

    pub fn stage(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["add", "--all", "--"];
        args.extend_from_slice(paths);
        self.run(&args).map(drop)
    }

    pub fn stage_all(&self) -> Result<()> {
        self.run(&["add", "--all"]).map(drop)
    }

    pub fn unstage(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let has_head = self.run(&["rev-parse", "--verify", "-q", "HEAD"]).is_ok();
        let mut args: Vec<&str> =
            if has_head { vec!["reset", "-q", "HEAD", "--"] } else { vec!["rm", "--cached", "-r", "-q", "--"] };
        args.extend_from_slice(paths);
        self.run(&args).map(drop)
    }

    pub fn unstage_all(&self) -> Result<()> {
        if self.run(&["rev-parse", "--verify", "-q", "HEAD"]).is_ok() {
            self.run(&["reset", "-q"]).map(drop)
        } else {
            self.run(&["rm", "--cached", "-r", "-q", "--", "."]).map(drop)
        }
    }

    pub fn apply_to_index(&self, patch: &str) -> Result<()> {
        self.run_with_stdin(&["apply", "--cached", "--recount", "--whitespace=nowarn", "-"], patch.as_bytes()).map(drop)
    }

    pub fn reverse_apply_to_worktree(&self, patch: &str) -> Result<()> {
        self.run_with_stdin(&["apply", "-R", "--recount", "--whitespace=nowarn", "-"], patch.as_bytes()).map(drop)
    }

    pub fn discard_tracked(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut args = vec!["checkout", "HEAD", "--"];
        args.extend_from_slice(paths);
        self.run(&args).map(drop)
    }

    pub fn commit(&self, message: &str, amend: bool) -> Result<String> {
        let mut args = vec!["commit", "-F", "-", "--cleanup=strip"];
        if amend {
            args.push("--amend");
        }
        if self.sign_commits {
            args.push("-S");
        }
        self.run_with_stdin(&args, message.as_bytes())?;
        Ok(self.run(&["rev-parse", "HEAD"])?.trim().to_string())
    }

    pub fn undo_last_commit(&self) -> Result<()> {
        if self.run(&["rev-parse", "--verify", "-q", "HEAD~1"]).is_ok() {
            self.run(&["reset", "--soft", "HEAD~1"]).map(drop)
        } else {
            self.run(&["update-ref", "-d", "HEAD"]).map(drop)
        }
    }

    pub fn checkout(&self, branch: &str) -> Result<()> {
        self.run(&["checkout", branch, "--"]).map(drop)
    }

    pub fn checkout_remote_branch(&self, remote_branch: &str, local_name: &str) -> Result<()> {
        self.run(&["checkout", "-b", local_name, "--track", remote_branch]).map(drop)
    }

    pub fn create_branch(&self, name: &str, start: Option<&str>, checkout: bool) -> Result<()> {
        let mut args = if checkout { vec!["checkout", "-b", name] } else { vec!["branch", name] };
        if let Some(start) = start {
            args.push(start);
        }
        self.run(&args).map(drop)
    }

    pub fn rename_branch(&self, old: &str, new: &str) -> Result<()> {
        self.run(&["branch", "-m", old, new]).map(drop)
    }

    pub fn delete_branch(&self, name: &str, force: bool) -> Result<()> {
        self.run(&["branch", if force { "-D" } else { "-d" }, name]).map(drop)
    }

    pub fn delete_remote_branch(&self, remote: &str, name: &str) -> Result<()> {
        self.run(&["push", remote, "--delete", name]).map(drop)
    }

    pub fn merge(&self, branch: &str, squash: bool) -> Result<String> {
        let mut args = vec!["merge", "--no-edit"];
        if squash {
            args.push("--squash");
        }
        args.push(branch);
        self.run(&args)
    }

    pub fn merge_abort(&self) -> Result<()> {
        self.run(&["merge", "--abort"]).map(drop)
    }

    pub fn checkout_conflict_side(&self, path: &str, ours: bool) -> Result<()> {
        self.run(&["checkout", if ours { "--ours" } else { "--theirs" }, "--", path])?;
        self.stage(&[path])
    }

    pub fn fetch(&self, remote: Option<&str>, prune: bool, progress: impl FnMut(Progress)) -> Result<()> {
        let mut args = vec!["fetch", "--progress"];
        if prune {
            args.push("--prune");
        }
        match remote {
            Some(r) => args.push(r),
            None => args.push("--all"),
        }
        self.run_with_progress(&args, progress).map(drop)
    }

    pub fn pull(&self, rebase: bool, progress: impl FnMut(Progress)) -> Result<()> {
        let args = ["pull", "--progress", if rebase { "--rebase" } else { "--no-rebase" }];
        self.run_with_progress(&args, progress).map(drop)
    }

    pub fn push(
        &self,
        remote: &str,
        branch: &str,
        set_upstream: bool,
        force_with_lease: bool,
        progress: impl FnMut(Progress),
    ) -> Result<()> {
        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        let mut args = vec!["push", "--progress"];
        if set_upstream {
            args.push("--set-upstream");
        }
        if force_with_lease {
            args.push("--force-with-lease");
        }
        args.extend([remote, refspec.as_str()]);
        self.run_with_progress(&args, progress).map(drop)
    }

    pub fn remotes(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .run(&["remote", "-v"])?
            .lines()
            .filter(|l| l.ends_with("(fetch)"))
            .filter_map(|l| {
                let mut parts = l.split_whitespace();
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            })
            .collect())
    }

    pub fn stash_push(&self, message: &str, paths: &[&str]) -> Result<()> {
        let mut args = vec!["stash", "push", "--include-untracked", "-m", message];
        if !paths.is_empty() {
            args.push("--");
            args.extend_from_slice(paths);
        }
        self.run(&args).map(drop)
    }

    pub fn stash_list(&self) -> Result<Vec<(String, String)>> {
        Ok(self
            .run(&["stash", "list", "--format=%gd%x00%gs"])?
            .lines()
            .filter_map(|l| l.split_once('\0').map(|(a, b)| (a.to_string(), b.to_string())))
            .collect())
    }

    pub fn stash_pop(&self, stash: &str) -> Result<()> {
        self.run(&["stash", "pop", stash]).map(drop)
    }

    pub fn stash_apply(&self, stash: &str) -> Result<()> {
        self.run(&["stash", "apply", stash]).map(drop)
    }

    pub fn stash_drop(&self, stash: &str) -> Result<()> {
        self.run(&["stash", "drop", stash]).map(drop)
    }

    pub fn config_get(&self, key: &str) -> Option<String> {
        self.run(&["config", "--get", key]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn config_set_global(&self, key: &str, value: &str) -> Result<()> {
        self.run(&["config", "--global", key, value]).map(drop)
    }

    pub fn has_hooks(&self, git_dir: &Path, hook: &str) -> bool {
        let dir =
            self.config_get("core.hooksPath").map(|p| self.workdir.join(p)).unwrap_or_else(|| git_dir.join("hooks"));
        dir.join(hook).is_file()
    }
}

fn parse_progress(line: &str) -> Progress {
    let line = line.trim().trim_start_matches("remote:").trim();
    let (phase, rest) = line.split_once(':').unwrap_or((line, ""));
    let percent = rest.split_whitespace().find_map(|w| w.strip_suffix('%')).and_then(|p| p.parse::<u8>().ok());
    Progress { phase: phase.trim().to_string(), percent, line: line.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_lines() {
        let p = parse_progress("Receiving objects:  45% (450/1000), 1.2 MiB | 2 MiB/s");
        assert_eq!(p.phase, "Receiving objects");
        assert_eq!(p.percent, Some(45));
        let p = parse_progress("remote: Counting objects: 100% (5/5), done.");
        assert_eq!(p.phase, "Counting objects");
        assert_eq!(p.percent, Some(100));
    }
}
