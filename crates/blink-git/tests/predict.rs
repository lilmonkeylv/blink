use std::fs;

use blink_git::{Error, GitCli, MergePrediction};

fn fixture() -> (tempfile::TempDir, GitCli) {
    let dir = tempfile::tempdir().unwrap();
    GitCli::init(dir.path(), "main").unwrap();
    let cli = GitCli::new(dir.path());
    for (k, v) in [
        ("user.name", "Test"),
        ("user.email", "t@example.com"),
        ("commit.gpgsign", "false"),
        ("core.autocrlf", "false"),
    ] {
        cli.run(&["config", k, v]).unwrap();
    }
    commit(&cli, dir.path(), &[("a.txt", "1\n2\n3\n"), ("b.txt", "b\n")], "base");
    (dir, cli)
}

fn commit(cli: &GitCli, root: &std::path::Path, files: &[(&str, &str)], message: &str) {
    for (path, content) in files {
        fs::write(root.join(path), content).unwrap();
    }
    cli.stage_all().unwrap();
    cli.commit(message, false).unwrap();
}

#[test]
fn predicts_conflicting_files_without_touching_the_worktree() {
    let (dir, cli) = fixture();
    cli.create_branch("side", None, true).unwrap();
    commit(&cli, dir.path(), &[("a.txt", "1\nside\n3\n"), ("c.txt", "new\n")], "side edits");
    cli.checkout("main").unwrap();
    commit(&cli, dir.path(), &[("a.txt", "1\nmain\n3\n"), ("b.txt", "main b\n")], "main edits");

    assert_eq!(cli.predict_merge("side").unwrap(), MergePrediction::Conflicts { files: vec!["a.txt".into()] });
    assert!(cli.run(&["status", "--porcelain"]).unwrap().is_empty());
    assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "1\nmain\n3\n");
}

#[test]
fn predicts_clean_merges() {
    let (dir, cli) = fixture();
    cli.create_branch("side", None, true).unwrap();
    commit(&cli, dir.path(), &[("c.txt", "side\n")], "side");
    cli.checkout("main").unwrap();
    commit(&cli, dir.path(), &[("b.txt", "main b\n")], "main");
    assert_eq!(cli.predict_merge("side").unwrap(), MergePrediction::Clean);
}

#[test]
fn recognises_up_to_date_and_fast_forward_merges() {
    let (dir, cli) = fixture();
    cli.create_branch("behind", None, false).unwrap();
    cli.create_branch("ahead", None, true).unwrap();
    commit(&cli, dir.path(), &[("c.txt", "ahead\n")], "ahead");
    cli.checkout("main").unwrap();

    assert_eq!(cli.predict_merge("behind").unwrap(), MergePrediction::UpToDate);
    assert_eq!(cli.predict_merge("ahead").unwrap(), MergePrediction::FastForward);
    cli.checkout("ahead").unwrap();
    assert_eq!(cli.predict_merge("main").unwrap(), MergePrediction::UpToDate);
}

#[test]
fn rejects_bad_branch_names() {
    let (_dir, cli) = fixture();
    assert!(matches!(cli.predict_merge("--output=x"), Err(Error::InvalidArgument(_))));
    assert!(matches!(cli.predict_merge("missing"), Err(Error::InvalidArgument(_))));
}

#[test]
fn unborn_head_has_no_prediction() {
    let dir = tempfile::tempdir().unwrap();
    GitCli::init(dir.path(), "main").unwrap();
    assert_eq!(GitCli::new(dir.path()).predict_merge("main").unwrap(), MergePrediction::Unknown);
}
