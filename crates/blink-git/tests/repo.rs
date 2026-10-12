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
