use std::fs;
use std::path::Path;

use blink_git::{Error, GitCli, HookEvent, HookResult, PushTarget};

fn fixture() -> (tempfile::TempDir, GitCli) {
    let dir = tempfile::tempdir().unwrap();
    GitCli::init(dir.path(), "main").unwrap();
    let cli = GitCli::new(dir.path());
    for (k, v) in [
        ("user.name", "Test"),
        ("user.email", "t@example.com"),
        ("commit.gpgsign", "false"),
        ("core.autocrlf", "false"),
        ("core.hooksPath", ".hooks"),
    ] {
        cli.run(&["config", k, v]).unwrap();
    }
    fs::create_dir_all(dir.path().join(".hooks")).unwrap();
    fs::write(dir.path().join("a"), "1\n").unwrap();
    cli.stage_all().unwrap();
    cli.commit("init", false).unwrap();
    (dir, cli)
}

fn hook(repo: &Path, name: &str, body: &str) {
    let path = repo.join(".hooks").join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn output(events: &[HookEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            HookEvent::Output { line } => Some(line.clone()),
            _ => None,
        })
        .collect()
}

fn results(events: &[HookEvent]) -> Vec<HookResult> {
    events
        .iter()
        .find_map(|e| match e {
            HookEvent::Finished { results } => Some(results.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn result(name: &str, code: i32) -> HookResult {
    HookResult { name: name.into(), code: Some(code) }
}

#[test]
fn streams_commit_hook_output_and_reports_each_hook() {
    let (dir, cli) = fixture();
    hook(dir.path(), "pre-commit", "echo 'lint: 2 files ok'\necho 'warning on stderr' >&2\nexit 0");
    hook(dir.path(), "commit-msg", "echo \"checking $(head -n1 \"$1\")\"");
    fs::write(dir.path().join("a"), "2\n").unwrap();
    cli.stage_all().unwrap();

    let mut events = Vec::new();
    let sha = cli.commit_with_hooks("second", false, true, |e| events.push(e)).unwrap();

    assert_eq!(sha.len(), 40);
    assert_eq!(events[0], HookEvent::Started { hooks: vec!["pre-commit".into(), "commit-msg".into()] });
    assert_eq!(output(&events), ["lint: 2 files ok", "warning on stderr", "checking second"]);
    assert_eq!(results(&events), [result("pre-commit", 0), result("commit-msg", 0)]);
}

#[test]
fn failing_pre_commit_hook_blocks_until_skipped() {
    let (dir, cli) = fixture();
    hook(dir.path(), "pre-commit", "echo 'src/a.rs:1: missing semicolon'\nexit 3");
    fs::write(dir.path().join("a"), "2\n").unwrap();
    cli.stage_all().unwrap();

    let mut events = Vec::new();
    let err = cli.commit_with_hooks("blocked", false, true, |e| events.push(e)).unwrap_err();
    let Error::HookFailed { hook, code, output: text } = err else { panic!("expected a hook failure, got {err:?}") };
    assert_eq!((hook.as_str(), code), ("pre-commit", Some(3)));
    assert!(text.contains("missing semicolon"));
    assert_eq!(results(&events), [result("pre-commit", 3)]);
    assert_eq!(cli.run(&["log", "-1", "--format=%s"]).unwrap().trim(), "init");

    let mut skipped = Vec::new();
    cli.commit_with_hooks("forced", false, false, |e| skipped.push(e)).unwrap();
    assert!(skipped.is_empty());
    assert_eq!(cli.run(&["log", "-1", "--format=%s"]).unwrap().trim(), "forced");
}

#[test]
fn commit_without_hooks_emits_nothing() {
    let (dir, cli) = fixture();
    fs::write(dir.path().join("a"), "2\n").unwrap();
    cli.stage_all().unwrap();
    let mut events = Vec::new();
    cli.commit_with_hooks("plain", false, true, |e| events.push(e)).unwrap();
    assert!(events.is_empty());
}

#[test]
fn other_commit_failures_are_not_blamed_on_hooks() {
    let (dir, cli) = fixture();
    hook(dir.path(), "pre-commit", "exit 0");
    let err = cli.commit_with_hooks("nothing staged", false, true, |_| {}).unwrap_err();
    assert!(matches!(err, Error::Cli { .. }), "{err:?}");
}

#[test]
fn pre_push_output_is_split_from_progress() {
    let (dir, cli) = fixture();
    let remote = tempfile::tempdir().unwrap();
    GitCli::new(remote.path()).run(&["init", "--bare", "-q"]).unwrap();
    cli.run(&["remote", "add", "origin", &remote.path().to_string_lossy()]).unwrap();
    hook(dir.path(), "pre-push", "echo \"pushing to $1\"\necho 'tests failed' >&2\nexit 1");
    let target = PushTarget::new_branch("origin", "main");

    let mut events = Vec::new();
    let err = cli.push_with_hooks(&target, false, true, |_| {}, |e| events.push(e)).unwrap_err();
    assert!(matches!(&err, Error::HookFailed { hook, .. } if hook == "pre-push"), "{err:?}");
    let mut lines = output(&events);
    lines.sort();
    assert_eq!(lines, ["pushing to origin", "tests failed"]);
    assert_eq!(results(&events), [result("pre-push", 1)]);

    let mut progress = Vec::new();
    let mut skipped = Vec::new();
    cli.push_with_hooks(&target, false, false, |p| progress.push(p.line), |e| skipped.push(e)).unwrap();
    assert!(skipped.is_empty());
    assert!(progress.iter().any(|l| l.starts_with("To ")), "{progress:?}");
    let pushed = GitCli::new(remote.path()).run(&["rev-parse", "main"]).unwrap();
    assert_eq!(pushed.trim(), cli.run(&["rev-parse", "HEAD"]).unwrap().trim());
}
