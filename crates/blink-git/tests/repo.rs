use std::fs;
use std::path::Path;

use blink_git::{BlobSource, ChangeKind, GitCli, HeadState, Repository};

fn fixture() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    GitCli::init(dir.path(), "main").unwrap();
    let cli = GitCli::new(dir.path());
    cli.run(&["config", "user.name", "Test"]).unwrap();
    cli.run(&["config", "user.email", "test@example.com"]).unwrap();
    cli.run(&["config", "commit.gpgsign", "false"]).unwrap();
    cli.run(&["config", "core.autocrlf", "false"]).unwrap();
    write(dir.path(), "a.txt", "one\ntwo\n");
    write(dir.path(), "b.txt", "bee\n");
    cli.stage_all().unwrap();
    cli.commit("initial\n\nbody text", false).unwrap();
    let repo = Repository::open(dir.path()).unwrap();
    (dir, repo)
}

fn write(root: &Path, path: &str, content: &str) {
    let full = root.join(path);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, content).unwrap();
}

fn kind_of(repo: &Repository, path: &str) -> Option<ChangeKind> {
    repo.status(None).unwrap().files.into_iter().find(|f| f.path == path).map(|f| f.kind())
}

#[test]
fn clean_repo_has_no_changes() {
    let (_dir, repo) = fixture();
    assert!(repo.status(None).unwrap().files.is_empty());
}

#[test]
fn status_detects_modified_untracked_deleted_and_staged() {
    let (dir, repo) = fixture();
    write(dir.path(), "a.txt", "one\nTWO\n");
    write(dir.path(), "new/c.txt", "sea\n");
    fs::remove_file(dir.path().join("b.txt")).unwrap();

    assert_eq!(kind_of(&repo, "a.txt"), Some(ChangeKind::Modified));
    assert_eq!(kind_of(&repo, "new/c.txt"), Some(ChangeKind::Untracked));
    assert_eq!(kind_of(&repo, "b.txt"), Some(ChangeKind::Deleted));

    repo.cli().stage(&["new/c.txt"]).unwrap();
    let status = repo.status(None).unwrap();
    let c = status.files.iter().find(|f| f.path == "new/c.txt").unwrap();
    assert_eq!(c.staged, Some(ChangeKind::Added));
    assert_eq!(c.unstaged, None);
}

#[test]
fn log_head_and_detail() {
    let (dir, repo) = fixture();
    write(dir.path(), "a.txt", "changed\n");
    repo.cli().stage_all().unwrap();
    let id = repo.cli().commit("second", false).unwrap();

    let log = repo.log(None, 0, 10).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].id, id);
    assert_eq!(log[0].summary, "second");
    assert_eq!(log[1].summary, "initial");
    assert_eq!(repo.log(None, 1, 10).unwrap().len(), 1);

    let detail = repo.commit_detail(&log[1].id).unwrap();
    assert_eq!(detail.body.trim(), "body text");
    let paths: Vec<_> = detail.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.txt", "b.txt"]);

    let detail = repo.commit_detail(&id).unwrap();
    assert_eq!(detail.files.len(), 1);
    assert_eq!(detail.files[0].kind, ChangeKind::Modified);

    assert!(matches!(repo.head().unwrap(), HeadState::Branch { name, .. } if name == "main"));
}

#[test]
fn branches_and_ahead_behind() {
    let (dir, repo) = fixture();
    let cli = repo.cli();
    cli.create_branch("feature", None, true).unwrap();
    write(dir.path(), "a.txt", "feature\n");
    cli.stage_all().unwrap();
    cli.commit("feature work", false).unwrap();

    let names: Vec<_> = repo.branches().unwrap().into_iter().map(|b| b.name).collect();
    assert!(names.contains(&"main".to_string()));
    assert!(names.contains(&"feature".to_string()));

    let ab = repo.ahead_behind("feature", "main").unwrap();
    assert_eq!((ab.ahead, ab.behind), (1, 0));
}

#[test]
fn reads_blobs_from_each_source() {
    let (dir, repo) = fixture();
    write(dir.path(), "a.txt", "worktree\n");
    assert_eq!(repo.read_blob(&BlobSource::Head, "a.txt").unwrap().unwrap(), b"one\ntwo\n");
    assert_eq!(repo.read_blob(&BlobSource::Index, "a.txt").unwrap().unwrap(), b"one\ntwo\n");
    assert_eq!(repo.read_blob(&BlobSource::Worktree, "a.txt").unwrap().unwrap(), b"worktree\n");
    assert!(repo.read_blob(&BlobSource::Head, "missing").unwrap().is_none());
}

#[test]
fn unborn_repository() {
    let dir = tempfile::tempdir().unwrap();
    GitCli::init(dir.path(), "trunk").unwrap();
    write(dir.path(), "x", "x\n");
    let repo = Repository::open(dir.path()).unwrap();
    assert_eq!(repo.head().unwrap(), HeadState::Unborn { name: "trunk".into() });
    assert!(repo.log(None, 0, 10).unwrap().is_empty());
    assert_eq!(kind_of(&repo, "x"), Some(ChangeKind::Untracked));
}

#[test]
fn undo_last_commit_keeps_changes() {
    let (dir, repo) = fixture();
    write(dir.path(), "a.txt", "undo me\n");
    repo.cli().stage_all().unwrap();
    repo.cli().commit("oops", false).unwrap();
    repo.cli().undo_last_commit().unwrap();
    assert_eq!(repo.log(None, 0, 10).unwrap().len(), 1);
    let status = repo.status(None).unwrap();
    assert_eq!(status.files[0].staged, Some(ChangeKind::Modified));
}

#[test]
fn commit_keeps_lines_that_look_like_comments() {
    let (dir, repo) = fixture();
    write(dir.path(), "a.txt", "changed\n");
    repo.cli().stage_all().unwrap();
    let message = "#123 fix crash\n\n# Notes\nbody";
    let id = repo.cli().commit(message, false).unwrap();
    assert_eq!(repo.cli().run(&["log", "-1", "--format=%B", &id]).unwrap().trim_end(), message);
}

#[test]
fn paths_are_not_treated_as_patterns() {
    let (dir, repo) = fixture();
    write(dir.path(), "[ab].txt", "literal\n");
    repo.cli().stage_all().unwrap();
    repo.cli().commit("add literal", false).unwrap();
    for path in ["a.txt", "b.txt", "[ab].txt"] {
        write(dir.path(), path, "dirty\n");
    }

    repo.cli().stage(&["[ab].txt"]).unwrap();
    let staged: Vec<_> =
        repo.status(None).unwrap().files.into_iter().filter(|f| f.staged.is_some()).map(|f| f.path).collect();
    assert_eq!(staged, ["[ab].txt"]);

    repo.cli().unstage(&["[ab].txt"]).unwrap();
    repo.cli().discard_tracked(&["[ab].txt"]).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("[ab].txt")).unwrap(), "literal\n");
    assert_eq!(kind_of(&repo, "a.txt"), Some(ChangeKind::Modified));
    assert_eq!(kind_of(&repo, "b.txt"), Some(ChangeKind::Modified));
}

#[test]
fn stages_more_paths_than_fit_on_a_command_line() {
    let (dir, repo) = fixture();
    let paths: Vec<String> =
        (0..1000).map(|i| format!("a/fairly/deeply/nested/directory/with-a-long-name-{i:04}.txt")).collect();
    for path in &paths {
        write(dir.path(), path, "x\n");
    }
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    repo.cli().stage(&refs).unwrap();
    let status = repo.status(None).unwrap();
    assert_eq!(status.files.iter().filter(|f| f.staged == Some(ChangeKind::Added)).count(), 1000);
}

#[test]
fn push_follows_the_configured_upstream() {
    let (dir, repo) = fixture();
    let remote = tempfile::tempdir().unwrap();
    GitCli::new(remote.path()).run(&["init", "--bare", "-b", "main"]).unwrap();
    let cli = repo.cli();
    cli.add_remote("origin", &remote.path().to_string_lossy()).unwrap();
    cli.run(&["push", "origin", "main:main", "main:feature-x"]).unwrap();
    cli.run(&["fetch", "origin"]).unwrap();
    cli.checkout_remote_branch("origin/feature-x", "fx").unwrap();
    write(dir.path(), "a.txt", "feature\n");
    cli.stage_all().unwrap();
    let id = cli.commit("feature work", false).unwrap();

    let target = cli.push_target("fx").unwrap();
    assert_eq!(target.remote, "origin");
    assert!(!target.set_upstream);
    cli.push(&target, false, |_| {}).unwrap();

    let bare = GitCli::new(remote.path());
    assert_eq!(bare.rev_parse("refs/heads/feature-x").unwrap(), id);
    assert!(bare.rev_parse("refs/heads/fx").is_err());

    cli.create_branch("untracked", None, true).unwrap();
    let target = cli.push_target("untracked").unwrap();
    assert_eq!(target, blink_git::PushTarget::new_branch("origin", "untracked"));
}

#[test]
fn remotes_are_read_from_config_and_track_changes() {
    let (_dir, repo) = fixture();
    assert!(!repo.has_remotes());
    let cli = repo.cli();
    cli.add_remote("origin", "git@github.com:acme/app.git").unwrap();
    assert_eq!(repo.remotes(), [("origin".to_string(), "git@github.com:acme/app.git".to_string())]);
    assert!(repo.has_remotes());
    cli.add_remote("upstream", "https://github.com/up/app.git").unwrap();
    assert_eq!(repo.remote_url("upstream").as_deref(), Some("https://github.com/up/app.git"));
    assert_eq!(repo.remote_url("missing"), None);
    let expected = cli.remotes().unwrap();
    let mut actual = repo.remotes();
    actual.sort();
    let mut expected = expected;
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn filters_ignored_paths_but_keeps_tracked_ones() {
    let (dir, repo) = fixture();
    write(dir.path(), ".gitignore", "target/\nnode_modules\n*.log\n!keep.log\n");
    write(dir.path(), "vendor/.gitignore", "cache/\n");
    write(dir.path(), "target/tracked.txt", "t\n");
    repo.cli().run(&["add", "-f", ".gitignore", "vendor/.gitignore", "target/tracked.txt"]).unwrap();
    write(dir.path(), "target/debug/build.o", "x");
    fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
    write(dir.path(), "vendor/cache/blob", "x");

    let mut paths: std::collections::BTreeSet<std::path::PathBuf> = [
        "target",
        "target/debug/build.o",
        "target/tracked.txt",
        "node_modules/pkg",
        "node_modules/pkg/index.js",
        "debug.log",
        "keep.log",
        "vendor/cache/blob",
        "vendor/lib.rs",
        "src/main.rs",
        ".gitignore",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .collect();
    repo.retain_unignored(&mut paths);
    let kept: Vec<_> = paths.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
    assert_eq!(kept, [".gitignore", "keep.log", "src/main.rs", "target/tracked.txt", "vendor/lib.rs"]);
}
