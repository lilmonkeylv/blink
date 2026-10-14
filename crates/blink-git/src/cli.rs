use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::redact::redact;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushTarget {
    pub remote: String,
    pub refspec: String,
    pub set_upstream: bool,
}

impl PushTarget {
    pub fn new_branch(remote: impl Into<String>, branch: &str) -> Self {
        Self { remote: remote.into(), refspec: format!("refs/heads/{branch}:refs/heads/{branch}"), set_upstream: true }
    }
}

#[derive(Debug, Clone)]
pub struct GitCli {
    workdir: PathBuf,
    git: PathBuf,
    env: Vec<(String, String)>,
    config: Vec<String>,
    sign_commits: bool,
    ssh_batch_mode: bool,
}

const NETWORK_COMMANDS: &[&str] = &["fetch", "pull", "push", "clone", "ls-remote"];
const STALL_GUARD: [&str; 4] = ["-c", "http.lowSpeedLimit=1000", "-c", "http.lowSpeedTime=60"];
const SSH_KEEPALIVE: &str = "-o ServerAliveInterval=15 -o ServerAliveCountMax=4";

static SYSTEM_GIT: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn find_git() -> Option<PathBuf> {
    if let Some(git) = crate::runtime::runtime().executable {
        return Some(git);
    }
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
            ssh_batch_mode: false,
        }
    }

    pub fn with_ssh_batch_mode(mut self) -> Self {
        self.ssh_batch_mode = true;
        self
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

    fn user_ssh_command(&self) -> bool {
        ["GIT_SSH_COMMAND", "GIT_SSH"]
            .iter()
            .any(|key| self.env.iter().any(|(k, _)| k == key) || std::env::var_os(key).is_some_and(|v| !v.is_empty()))
            || self.config_get("core.sshCommand").is_some()
    }

    fn ssh_command(&self, program: Option<&Path>) -> String {
        // GIT_SSH_COMMAND outranks GIT_SSH, so a chosen ssh binary has to be named here to keep the keepalive options.
        let program =
            program.map_or_else(|| "ssh".to_string(), |p| format!("'{}'", p.to_string_lossy().replace('\\', "/")));
        let batch = if self.ssh_batch_mode { " -o BatchMode=yes" } else { "" };
        format!("{program} {SSH_KEEPALIVE}{batch}")
    }

    pub(crate) fn command(&self, args: &[&str]) -> Command {
        let network = args.first().is_some_and(|sub| NETWORK_COMMANDS.contains(sub));
        let runtime = crate::runtime::runtime();
        let mut cmd = Command::new(&self.git);
        cmd.current_dir(&self.workdir)
            .args(["-c", "core.quotepath=false", "-c", "color.ui=false"])
            .args(runtime.config_args())
            .args(self.config.iter().flat_map(|c| ["-c", c.as_str()]));
        if network {
            cmd.args(STALL_GUARD);
            if !self.user_ssh_command() {
                cmd.env("GIT_SSH_COMMAND", self.ssh_command(runtime.ssh.as_deref()));
            }
        }
        cmd.args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("LC_ALL", "C")
            .envs(runtime.env())
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() };
            Err(Error::Cli {
                command: args.first().copied().unwrap_or_default().to_string(),
                stderr: redact(message),
                code: output.status.code(),
            })
        }
    }

    fn spawn_error(e: std::io::Error) -> Error {
        if e.kind() == std::io::ErrorKind::NotFound { Error::GitNotFound } else { e.into() }
    }

    pub fn run(&self, args: &[&str]) -> Result<String> {
        retry_when_locked(|| {
            let output = self.command(args).output().map_err(Self::spawn_error)?;
            Self::finish(args, output)
        })
    }

    pub fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = self.command(args).output().map_err(Self::spawn_error)?;
        if output.status.success() {
            return Ok(output.stdout);
        }
        Self::finish(args, output).map(String::into_bytes)
    }

    pub fn run_with_stdin(&self, args: &[&str], input: &[u8]) -> Result<String> {
        retry_when_locked(|| {
            let mut child = self.command(args).stdin(Stdio::piped()).spawn().map_err(Self::spawn_error)?;
            let mut stdin = child.stdin.take().expect("stdin piped");
            let payload = input.to_vec();
            let writer = std::thread::spawn(move || stdin.write_all(&payload));
            let output = child.wait_with_output()?;
            writer.join().map_err(|_| Error::Git("stdin writer panicked".into()))??;
            Self::finish(args, output)
        })
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

    pub(crate) fn literal_pathspecs(&self) -> Self {
        self.clone().with_env("GIT_LITERAL_PATHSPECS", "1")
    }

    fn run_with_pathspecs(&self, args: &[&str], paths: &[&str]) -> Result<String> {
        let mut args = args.to_vec();
        args.extend(["--pathspec-from-file=-", "--pathspec-file-nul"]);
        let input: Vec<u8> = paths.iter().flat_map(|p| p.bytes().chain([0])).collect();
        self.literal_pathspecs().run_with_stdin(&args, &input)
    }

    pub fn stage(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        self.run_with_pathspecs(&["add", "--all"], paths).map(drop)
    }

    pub fn stage_all(&self) -> Result<()> {
        self.run(&["add", "--all"]).map(drop)
    }

    pub fn unstage(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let has_head = self.run(&["rev-parse", "--verify", "-q", "HEAD"]).is_ok();
        let args: &[&str] = if has_head { &["reset", "-q", "HEAD"] } else { &["rm", "--cached", "-r", "-q"] };
        self.run_with_pathspecs(args, paths).map(drop)
    }

    pub fn unstage_all(&self) -> Result<()> {
        if self.run(&["rev-parse", "--verify", "-q", "HEAD"]).is_ok() {
            self.run(&["reset", "-q"]).map(drop)
        } else {
            self.run(&["rm", "--cached", "-r", "-q", "--", "."]).map(drop)
        }
    }

    pub fn write_tree(&self) -> Result<String> {
        Ok(self.run(&["write-tree"])?.trim().to_string())
    }

    pub fn read_tree(&self, tree: &str) -> Result<()> {
        self.run(&["read-tree", "--reset", tree]).map(drop)
    }

    pub fn apply_to_index(&self, patch: &[u8]) -> Result<()> {
        self.run_with_stdin(&["apply", "--cached", "--recount", "--whitespace=nowarn", "-"], patch).map(drop)
    }

    pub fn reverse_apply_to_worktree(&self, patch: &[u8]) -> Result<()> {
        self.run_with_stdin(&["apply", "-R", "--recount", "--whitespace=nowarn", "-"], patch).map(drop)
    }

    pub fn discard_tracked(&self, paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }
        self.run_with_pathspecs(&["checkout", "HEAD"], paths).map(drop)
    }

    pub fn commit(&self, message: &str, amend: bool) -> Result<String> {
        let mut args = vec!["commit", "-F", "-", "--cleanup=whitespace"];
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

    pub fn set_upstream(&self, branch: &str, upstream: Option<&str>) -> Result<()> {
        match upstream {
            Some(upstream) => self.run(&["branch", &format!("--set-upstream-to={upstream}"), branch]),
            None => self.run(&["branch", "--unset-upstream", branch]),
        }
        .map(drop)
    }

    pub fn prune_remote(&self, remote: &str) -> Result<Vec<String>> {
        Ok(parse_pruned(&self.run(&["remote", "prune", remote])?))
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
        self.literal_pathspecs().run(&["checkout", if ours { "--ours" } else { "--theirs" }, "--", path])?;
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

    pub fn push_target(&self, branch: &str) -> Result<PushTarget> {
        let remote = self.config_get(&format!("branch.{branch}.remote")).filter(|r| r != ".");
        let merge = self.config_get(&format!("branch.{branch}.merge"));
        if let (Some(remote), Some(merge)) = (remote, merge) {
            return Ok(PushTarget { remote, refspec: format!("refs/heads/{branch}:{merge}"), set_upstream: false });
        }
        if let Some(remote) = self.config_get("remote.pushDefault") {
            return Ok(PushTarget::new_branch(remote, branch));
        }
        let mut names: Vec<String> = self.remotes()?.into_iter().map(|(name, _)| name).collect();
        let remote = match names.iter().position(|n| n == "origin") {
            Some(origin) => names.swap_remove(origin),
            None if names.len() == 1 => names.remove(0),
            None if names.is_empty() => return Err(Error::Git("This repository has no remote to push to".into())),
            None => return Err(Error::Git("This branch has no upstream and no origin remote to push to".into())),
        };
        Ok(PushTarget::new_branch(remote, branch))
    }

    pub fn push(&self, target: &PushTarget, force_with_lease: bool, progress: impl FnMut(Progress)) -> Result<()> {
        let mut args = vec!["push", "--progress"];
        if target.set_upstream {
            args.push("--set-upstream");
        }
        if force_with_lease {
            args.push("--force-with-lease");
        }
        args.extend([target.remote.as_str(), target.refspec.as_str()]);
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

    pub fn stash_staged(&self, message: &str) -> Result<()> {
        self.run(&["stash", "push", "--staged", "-m", message]).map(drop)
    }

    pub fn stash_push(&self, message: &str, paths: &[&str]) -> Result<()> {
        let args = ["stash", "push", "--include-untracked", "-m", message];
        if paths.is_empty() {
            return self.run(&args).map(drop);
        }
        self.run_with_pathspecs(&args, paths).map(drop)
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

fn parse_pruned(output: &str) -> Vec<String> {
    output.lines().filter_map(|l| l.trim().strip_prefix("* [pruned] ")).map(str::to_string).collect()
}

const LOCK_RETRY_DELAYS_MS: [u64; 5] = [100, 200, 400, 800, 1500];

pub fn is_lock_error(stderr: &str) -> bool {
    stderr.contains(".lock") && (stderr.contains("File exists") || stderr.contains("Another git process"))
}

pub fn locked_file(stderr: &str) -> Option<&str> {
    let start = stderr.find("Unable to create '")? + "Unable to create '".len();
    let end = start + stderr[start..].find('\'')?;
    Some(&stderr[start..end]).filter(|p| p.ends_with(".lock"))
}

fn retry_when_locked<T>(mut attempt: impl FnMut() -> Result<T>) -> Result<T> {
    for delay in LOCK_RETRY_DELAYS_MS {
        match attempt() {
            Err(Error::Cli { stderr, .. }) if is_lock_error(&stderr) => {
                std::thread::sleep(Duration::from_millis(delay))
            }
            other => return other,
        }
    }
    attempt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCKED: &str = "fatal: Unable to create 'C:/work/repo/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository";

    #[test]
    fn recognises_lock_contention() {
        assert!(is_lock_error(LOCKED));
        assert_eq!(locked_file(LOCKED), Some("C:/work/repo/.git/index.lock"));
        assert!(!is_lock_error("error: pathspec 'x' did not match any file(s) known to git"));
        assert_eq!(locked_file("fatal: Unable to create '/tmp/x': File exists."), None);
    }

    #[test]
    fn retries_until_the_lock_is_released() {
        let mut calls = 0;
        let result = retry_when_locked(|| {
            calls += 1;
            if calls < 3 {
                Err(Error::Cli { command: "checkout".into(), stderr: LOCKED.into(), code: Some(128) })
            } else {
                Ok(calls)
            }
        });
        assert_eq!(result.unwrap(), 3);
    }

    #[test]
    fn parses_pruned_refs() {
        let out = "Pruning origin\nURL: https://github.com/a/b.git\n * [pruned] origin/old-feature\n * [pruned] origin/fix/x\n";
        assert_eq!(parse_pruned(out), ["origin/old-feature", "origin/fix/x"]);
        assert!(parse_pruned("").is_empty());
    }

    fn env_of(cmd: &Command, key: &str) -> Option<String> {
        cmd.get_envs().find(|(k, _)| *k == key).and_then(|(_, v)| v).map(|v| v.to_string_lossy().into_owned())
    }

    #[test]
    fn network_commands_get_stall_and_keepalive_guards() {
        let dir = tempfile::tempdir().unwrap();
        GitCli::init(dir.path(), "main").unwrap();
        let cli = GitCli::new(dir.path());
        let args: Vec<String> =
            cli.command(&["fetch", "--all"]).get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.windows(2).any(|w| w == ["-c", "http.lowSpeedLimit=1000"]));
        assert!(args.windows(2).any(|w| w == ["-c", "http.lowSpeedTime=60"]));
        assert!(args.iter().position(|a| a == "http.lowSpeedTime=60") < args.iter().position(|a| a == "fetch"));

        let status: Vec<String> =
            cli.command(&["status"]).get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(!status.iter().any(|a| a.starts_with("http.")));
        assert_eq!(env_of(&cli.command(&["status"]), "GIT_SSH_COMMAND"), None);

        if !cli.user_ssh_command() {
            let ssh = env_of(&cli.command(&["push"]), "GIT_SSH_COMMAND").unwrap();
            assert!(ssh.contains("ServerAliveInterval=15") && ssh.contains("ServerAliveCountMax=4"));
            assert!(!ssh.contains("BatchMode"));
            let batch = cli.clone().with_ssh_batch_mode();
            assert!(env_of(&batch.command(&["fetch"]), "GIT_SSH_COMMAND").unwrap().contains("BatchMode=yes"));
        }

        cli.run(&["config", "core.sshCommand", "ssh -i custom"]).unwrap();
        assert_eq!(env_of(&cli.command(&["fetch"]), "GIT_SSH_COMMAND"), None);
        let custom = GitCli::new(dir.path()).with_env("GIT_SSH_COMMAND", "plink");
        assert_eq!(env_of(&custom.command(&["fetch"]), "GIT_SSH_COMMAND").as_deref(), Some("plink"));
    }

    #[test]
    fn chosen_ssh_binary_keeps_the_keepalive_options() {
        let cli = GitCli::new(".");
        assert_eq!(cli.ssh_command(None), "ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=4");
        let system = Path::new(r"C:\Windows\System32\OpenSSH\ssh.exe");
        assert_eq!(
            cli.clone().with_ssh_batch_mode().ssh_command(Some(system)),
            "'C:/Windows/System32/OpenSSH/ssh.exe' -o ServerAliveInterval=15 -o ServerAliveCountMax=4 -o BatchMode=yes"
        );
    }

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
