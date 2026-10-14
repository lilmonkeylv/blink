use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

use crate::cli::{GitCli, Progress, PushTarget, parse_progress, retry_when_locked};
use crate::error::{Error, Result};
use crate::redact::redact;

const COMMIT_HOOKS: [&str; 4] = ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"];
const SKIPPED_BY_NO_VERIFY: [&str; 3] = ["pre-commit", "commit-msg", "pre-push"];
const BLOCKING_HOOKS: [&str; 4] = ["pre-commit", "prepare-commit-msg", "commit-msg", "pre-push"];
const PUSH_OUTPUT: [&str; 11] = [
    "Enumerating objects",
    "Counting objects",
    "Delta compression",
    "Compressing objects",
    "Writing objects",
    "Total ",
    "remote:",
    "To ",
    "Everything up-to-date",
    "error: failed to push",
    "error: src refspec",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResult {
    pub name: String,
    pub code: Option<i32>,
}

impl HookResult {
    pub fn passed(&self) -> bool {
        self.code == Some(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum HookEvent {
    Started { hooks: Vec<String> },
    Output { line: String },
    Finished { results: Vec<HookResult> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

struct Traced {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    hooks: Vec<HookResult>,
}

#[derive(Deserialize)]
struct TraceEvent {
    event: String,
    sid: String,
    child_id: Option<u64>,
    child_class: Option<String>,
    hook_name: Option<String>,
    code: Option<i32>,
}

impl GitCli {
    pub fn hooks_dir(&self) -> Option<PathBuf> {
        let path = self.run(&["rev-parse", "--git-path", "hooks"]).ok()?;
        Some(self.workdir().join(path.trim()))
    }

    pub fn installed_hooks(&self, names: &[&str]) -> Vec<String> {
        let Some(dir) = self.hooks_dir() else { return Vec::new() };
        names.iter().filter(|name| dir.join(name).is_file()).map(|name| name.to_string()).collect()
    }

    pub fn commit_with_hooks(
        &self,
        message: &str,
        amend: bool,
        verify: bool,
        mut on_event: impl FnMut(HookEvent),
    ) -> Result<String> {
        let mut args = vec!["commit", "-F", "-", "--cleanup=whitespace"];
        if amend {
            args.push("--amend");
        }
        if self.signs_commits() {
            args.push("-S");
        }
        if !verify {
            args.push("--no-verify");
        }
        let hooks = self.expected_hooks(&COMMIT_HOOKS, verify);
        retry_when_locked(|| {
            if !hooks.is_empty() {
                on_event(HookEvent::Started { hooks: hooks.clone() });
            }
            let traced = self.run_traced(&args, Some(message.as_bytes()), |stream, line| {
                if stream == Stream::Stderr && !hooks.is_empty() {
                    on_event(HookEvent::Output { line: line.to_string() });
                }
            })?;
            if !hooks.is_empty() || !traced.hooks.is_empty() {
                on_event(HookEvent::Finished { results: traced.hooks.clone() });
            }
            Self::settle(&args, traced)
        })?;
        Ok(self.run(&["rev-parse", "HEAD"])?.trim().to_string())
    }

    pub fn push_with_hooks(
        &self,
        target: &PushTarget,
        force_with_lease: bool,
        verify: bool,
        progress: impl FnMut(Progress),
        on_event: impl FnMut(HookEvent),
    ) -> Result<()> {
        let mut args = vec!["push", "--progress"];
        if target.set_upstream {
            args.push("--set-upstream");
        }
        if force_with_lease {
            args.push("--force-with-lease");
        }
        if !verify {
            args.push("--no-verify");
        }
        args.extend([target.remote.as_str(), target.refspec.as_str()]);
        self.push_traced(&args, verify, progress, on_event)
    }

    pub fn push_tags_with_hooks(
        &self,
        remote: &str,
        verify: bool,
        progress: impl FnMut(Progress),
        on_event: impl FnMut(HookEvent),
    ) -> Result<()> {
        let mut args = vec!["push", "--progress"];
        if !verify {
            args.push("--no-verify");
        }
        args.extend([remote, "--tags"]);
        self.push_traced(&args, verify, progress, on_event)
    }

    fn push_traced(
        &self,
        args: &[&str],
        verify: bool,
        mut progress: impl FnMut(Progress),
        mut on_event: impl FnMut(HookEvent),
    ) -> Result<()> {
        let hooks = self.expected_hooks(&["pre-push"], verify);
        let mut in_hook = !hooks.is_empty();
        if in_hook {
            on_event(HookEvent::Started { hooks: hooks.clone() });
        }
        let expects_hook = in_hook;
        let traced = self.run_traced(args, None, |stream, line| {
            let hook_line = match stream {
                Stream::Stdout => expects_hook,
                Stream::Stderr => in_hook && !PUSH_OUTPUT.iter().any(|prefix| line.trim_start().starts_with(prefix)),
            };
            if hook_line {
                on_event(HookEvent::Output { line: line.to_string() });
            } else if stream == Stream::Stderr {
                in_hook = false;
                progress(parse_progress(line));
            }
        })?;
        if !hooks.is_empty() || !traced.hooks.is_empty() {
            on_event(HookEvent::Finished { results: traced.hooks.clone() });
        }
        Self::settle(args, traced).map(drop)
    }

    fn expected_hooks(&self, names: &[&str], verify: bool) -> Vec<String> {
        let mut hooks = self.installed_hooks(names);
        if !verify {
            hooks.retain(|h| !SKIPPED_BY_NO_VERIFY.contains(&h.as_str()));
        }
        hooks
    }

    fn settle(args: &[&str], traced: Traced) -> Result<String> {
        if !traced.status.success()
            && let Some(failed) = traced.hooks.iter().find(|h| !h.passed() && BLOCKING_HOOKS.contains(&h.name.as_str()))
        {
            let output = [&traced.stdout, &traced.stderr]
                .map(|bytes| String::from_utf8_lossy(bytes).trim().to_string())
                .into_iter()
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            return Err(Error::HookFailed { hook: failed.name.clone(), code: failed.code, output: redact(&output) });
        }
        let output = std::process::Output { status: traced.status, stdout: traced.stdout, stderr: traced.stderr };
        Self::finish(args, output)
    }

    fn run_traced(&self, args: &[&str], input: Option<&[u8]>, mut on_line: impl FnMut(Stream, &str)) -> Result<Traced> {
        let trace = tempfile::tempdir()?;
        let mut cmd = self.command(args);
        cmd.env("GIT_TRACE2_EVENT", trace.path()).env("GIT_TRACE2_EVENT_BRIEF", "true");
        if input.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().map_err(Self::spawn_error)?;
        let writer = match (input, child.stdin.take()) {
            (Some(input), Some(mut stdin)) => {
                let payload = input.to_vec();
                Some(std::thread::spawn(move || stdin.write_all(&payload)))
            }
            _ => None,
        };
        let (lines, received) = mpsc::channel();
        let stdout_reader = forward_lines(child.stdout.take().expect("stdout piped"), Stream::Stdout, lines.clone());
        let stderr_reader = forward_lines(child.stderr.take().expect("stderr piped"), Stream::Stderr, lines);
        for (stream, line) in received {
            on_line(stream, &line);
        }
        let status = child.wait()?;
        let stdout = stdout_reader.join().map_err(|_| Error::Git("stdout reader panicked".into()))??;
        let stderr = stderr_reader.join().map_err(|_| Error::Git("stderr reader panicked".into()))??;
        if let Some(writer) = writer {
            // git may exit before reading stdin, e.g. when a hook rejects the commit
            let _ = writer.join();
        }
        Ok(Traced { status, stdout, stderr, hooks: hook_results(trace.path()) })
    }
}

fn forward_lines(
    mut source: impl Read + Send + 'static,
    stream: Stream,
    lines: mpsc::Sender<(Stream, String)>,
) -> std::thread::JoinHandle<Result<Vec<u8>>> {
    std::thread::spawn(move || {
        read_lines(&mut source, |line| {
            let _ = lines.send((stream, line.to_string()));
        })
    })
}

fn read_lines(source: &mut impl Read, mut on_line: impl FnMut(&str)) -> Result<Vec<u8>> {
    let mut all = Vec::new();
    let mut line = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = source.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        for &b in &chunk[..n] {
            all.push(b);
            if b == b'\r' || b == b'\n' {
                if !line.is_empty() {
                    on_line(&String::from_utf8_lossy(&line));
                    line.clear();
                }
            } else {
                line.push(b);
            }
        }
    }
    if !line.is_empty() {
        on_line(&String::from_utf8_lossy(&line));
    }
    Ok(all)
}

fn hook_results(trace_dir: &Path) -> Vec<HookResult> {
    let mut events: Vec<TraceEvent> = std::fs::read_dir(trace_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .flat_map(|text| text.lines().filter_map(|l| serde_json::from_str::<TraceEvent>(l).ok()).collect::<Vec<_>>())
        .collect();
    events.retain(|e| !e.sid.contains('/'));
    parse_hook_events(&events)
}

fn parse_hook_events(events: &[TraceEvent]) -> Vec<HookResult> {
    let mut running: Vec<(u64, usize)> = Vec::new();
    let mut results = Vec::new();
    for event in events {
        match (event.event.as_str(), event.child_id) {
            ("child_start", Some(id)) if event.child_class.as_deref() == Some("hook") => {
                let name = event.hook_name.clone().unwrap_or_else(|| "hook".into());
                running.push((id, results.len()));
                results.push(HookResult { name, code: None });
            }
            ("child_exit", Some(id)) => {
                if let Some(&(_, index)) = running.iter().find(|(child, _)| *child == id) {
                    results[index].code = event.code;
                }
            }
            _ => {}
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(json: &str) -> TraceEvent {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn pairs_hook_starts_with_their_exits() {
        let events = [
            event(r#"{"event":"child_start","sid":"s","child_id":0,"child_class":"hook","hook_name":"pre-commit"}"#),
            event(r#"{"event":"child_exit","sid":"s","child_id":0,"pid":7,"code":0}"#),
            event(r#"{"event":"child_start","sid":"s","child_id":1,"child_class":"?","argv":["git","gc"]}"#),
            event(r#"{"event":"child_exit","sid":"s","child_id":1,"code":2}"#),
            event(r#"{"event":"child_start","sid":"s","child_id":2,"child_class":"hook","hook_name":"commit-msg"}"#),
            event(r#"{"event":"child_exit","sid":"s","child_id":2,"code":1}"#),
        ];
        assert_eq!(
            parse_hook_events(&events),
            [
                HookResult { name: "pre-commit".into(), code: Some(0) },
                HookResult { name: "commit-msg".into(), code: Some(1) }
            ]
        );
    }

    #[test]
    fn splits_output_on_carriage_returns_and_newlines() {
        let mut lines = Vec::new();
        let all = read_lines(&mut &b"one\r\ntwo\rthree\nlast"[..], |l| lines.push(l.to_string())).unwrap();
        assert_eq!(lines, ["one", "two", "three", "last"]);
        assert_eq!(all.len(), 19);
    }
}
