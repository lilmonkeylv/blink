use std::fs;

use blink_git::{BlobSource, GitCli, Operation, RebaseAction, RebaseStep, Repository, ResetMode, SearchKind};

fn fixture(commits: &[(&str, &str, &str)]) -> (tempfile::TempDir, Repository, Vec<String>) {
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
    let mut ids = Vec::new();
    for (path, content, message) in commits {
        fs::write(dir.path().join(path), content).unwrap();
        cli.stage_all().unwrap();
        ids.push(cli.commit(message, false).unwrap());
    }
    let repo = Repository::open(dir.path()).unwrap();
    (dir, repo, ids)
}

fn summaries(repo: &Repository) -> Vec<String> {
    repo.log(None, 0, 100).unwrap().into_iter().map(|c| c.summary).collect()
}

#[test]
fn revert_and_cherry_pick() {
    let (_dir, repo, ids) = fixture(&[("a", "1\n", "one"), ("a", "2\n", "two")]);
    let cli = repo.cli();
    cli.revert(&ids[1]).unwrap();
    assert_eq!(repo.read_blob(&BlobSource::Head, "a").unwrap().unwrap(), b"1\n");

    cli.create_branch("side", Some(&ids[0]), true).unwrap();
    cli.cherry_pick(&[&ids[1]]).unwrap();
    assert_eq!(summaries(&repo), ["two", "one"]);
}

#[test]
fn reset_modes() {
    let (dir, repo, ids) = fixture(&[("a", "1\n", "one"), ("a", "2\n", "two")]);
    repo.cli().reset(&ids[0], ResetMode::Soft).unwrap();
    assert_eq!(repo.status(None).unwrap().files.len(), 1);
    repo.cli().reset(&ids[0], ResetMode::Hard).unwrap();
    assert!(repo.status(None).unwrap().files.is_empty());
    assert_eq!(fs::read(dir.path().join("a")).unwrap(), b"1\n");
}

#[test]
fn interactive_rebase_reorders_squashes_rewords_and_drops() {
    let (_dir, repo, ids) =
        fixture(&[("a", "a\n", "base"), ("b", "b\n", "add b"), ("c", "c\n", "add c"), ("d", "d\n", "add d")]);
    let step = |action, i: usize, message: Option<&str>| RebaseStep {
        action,
        sha: ids[i].clone(),
        message: message.map(str::to_string),
    };
    repo.cli()
        .rebase_interactive(
            Some(&ids[0]),
            &[
                step(RebaseAction::Pick, 2, None),
                step(RebaseAction::Reword, 1, Some("#12 renamed b")),
                step(RebaseAction::Drop, 3, None),
            ],
        )
        .unwrap();
    assert_eq!(summaries(&repo), ["#12 renamed b", "add c", "base"]);

    let ids: Vec<String> = repo.log(None, 0, 10).unwrap().into_iter().map(|c| c.id).collect();
    repo.cli()
        .rebase_interactive(
            Some(&ids[2]),
            &[
                RebaseStep { action: RebaseAction::Pick, sha: ids[1].clone(), message: None },
                RebaseStep { action: RebaseAction::Squash, sha: ids[0].clone(), message: Some("combined".into()) },
            ],
        )
        .unwrap();
    assert_eq!(summaries(&repo), ["combined", "base"]);
    assert!(repo.read_blob(&BlobSource::Head, "b").unwrap().is_some());
    assert!(repo.read_blob(&BlobSource::Head, "c").unwrap().is_some());
}

#[test]
fn conflicting_cherry_pick_can_be_aborted() {
    let (_dir, repo, ids) = fixture(&[("a", "base\n", "base"), ("a", "main\n", "main change")]);
    let cli = repo.cli();
    cli.create_branch("side", Some(&ids[0]), true).unwrap();
    fs::write(repo.workdir().join("a"), "side\n").unwrap();
    cli.stage_all().unwrap();
    cli.commit("side change", false).unwrap();
    assert!(cli.cherry_pick(&[&ids[1]]).is_err());
    assert_eq!(repo.is_operation_in_progress(), Some("cherry-pick"));
    cli.abort_operation(Operation::CherryPick).unwrap();
    assert_eq!(repo.is_operation_in_progress(), None);
}

#[test]
fn tags_blame_history_search_reflog() {
    let (_dir, repo, ids) = fixture(&[("f", "one\n", "first"), ("f", "one\ntwo\n", "second needle")]);
    let cli = repo.cli();
    cli.create_tag("v1", &ids[0], Some("release")).unwrap();
    assert_eq!(cli.tags().unwrap(), [("v1".to_string(), ids[0].clone())]);
    cli.delete_tag("v1").unwrap();
    assert!(cli.tags().unwrap().is_empty());

    let blame = cli.blame("f", None).unwrap();
    assert_eq!(blame.lines.len(), 2);
    assert_eq!(blame.lines[0].sha, ids[0]);
    assert_eq!(blame.commits[&ids[1]].summary, "second needle");

    assert_eq!(cli.file_history("f", 0, 10).unwrap().len(), 2);
    assert_eq!(cli.search(SearchKind::Message, "NEEDLE", 10).unwrap()[0].id, ids[1]);
    assert_eq!(cli.search(SearchKind::Content, "two", 10).unwrap().len(), 1);
    assert_eq!(cli.search(SearchKind::Message, &ids[0][..8], 10).unwrap()[0].id, ids[0]);
    assert!(!cli.reflog(10).unwrap().is_empty());
    assert_eq!(repo.range_files(&ids[0], &ids[1]).unwrap().len(), 1);
}

#[test]
fn worktrees_and_patches() {
    let (dir, repo, ids) = fixture(&[("f", "one\n", "first"), ("f", "two\n", "second")]);
    let cli = repo.cli();
    let wt = dir.path().with_extension("wt");
    cli.add_worktree(&wt, "feature", true).unwrap();
    let list = cli.worktrees().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1].branch.as_deref(), Some("feature"));
    cli.remove_worktree(&wt.to_string_lossy(), true).unwrap();

    let patch = cli.format_patch(&ids[1]).unwrap();
    cli.reset(&ids[0], ResetMode::Hard).unwrap();
    cli.apply_patch(&patch).unwrap();
    assert_eq!(fs::read(dir.path().join("f")).unwrap(), b"two\n");
}

#[test]
fn clone_local_repository() {
    let (dir, _repo, _) = fixture(&[("f", "x\n", "first")]);
    let dest = dir.path().with_extension("clone");
    let url = dir.path().to_string_lossy().into_owned();
    let mut events = 0;
    GitCli::clone_repository(&url, &dest, None, false, &GitCli::new("."), |_| events += 1).unwrap();
    let cloned = Repository::open(&dest).unwrap();
    assert_eq!(cloned.log(None, 0, 10).unwrap().len(), 1);
    let _ = fs::remove_dir_all(dest);
}

#[test]
fn partial_clone_reads_old_blobs_on_demand() {
    let (dir, repo, ids) = fixture(&[("f", "old\n", "first"), ("f", "new\n", "second")]);
    repo.cli().run(&["config", "uploadpack.allowFilter", "true"]).unwrap();
    let dest = dir.path().with_extension("partial");
    let url = format!("file://{}", dir.path().to_string_lossy().replace('\\', "/"));
    GitCli::clone_repository(&url, &dest, None, true, &GitCli::new("."), |_| {}).unwrap();
    let cloned = Repository::open(&dest).unwrap();
    assert_eq!(cloned.cli().run(&["config", "remote.origin.promisor"]).unwrap().trim(), "true");
    let old = cloned.read_blob(&BlobSource::Revision(ids[0].clone()), "f").unwrap().unwrap();
    assert_eq!(old, b"old\n");
    let _ = fs::remove_dir_all(dest);
}

#[test]
fn sparse_checkout_round_trip() {
    let (dir, repo, _) = fixture(&[("root.txt", "r\n", "root")]);
    let cli = repo.cli();
    for folder in ["keep", "drop"] {
        fs::create_dir(dir.path().join(folder)).unwrap();
        fs::write(dir.path().join(folder).join("f"), "x\n").unwrap();
    }
    cli.stage_all().unwrap();
    cli.commit("folders", false).unwrap();

    assert_eq!(cli.sparse_folders(), None);
    cli.set_sparse_folders(Some(&["keep".to_string()])).unwrap();
    assert_eq!(cli.sparse_folders(), Some(vec!["keep".to_string()]));
    assert!(dir.path().join("keep/f").exists());
    assert!(!dir.path().join("drop/f").exists());
    assert!(dir.path().join("root.txt").exists());

    cli.set_sparse_folders(None).unwrap();
    assert_eq!(cli.sparse_folders(), None);
    assert!(dir.path().join("drop/f").exists());
}

#[test]
fn checkout_waits_out_a_short_lived_index_lock() {
    let (dir, repo, _) = fixture(&[("f", "x\n", "first")]);
    let cli = repo.cli();
    cli.create_branch("other", None, false).unwrap();
    let lock = dir.path().join(".git").join("index.lock");
    fs::write(&lock, "").unwrap();
    let releaser = {
        let lock = lock.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(250));
            fs::remove_file(lock).unwrap();
        })
    };
    cli.checkout("other").unwrap();
    releaser.join().unwrap();
    assert_eq!(repo.head().unwrap().branch_name(), Some("other"));

    fs::write(&lock, "").unwrap();
    let err = cli.checkout("main").unwrap_err();
    assert!(matches!(err, blink_git::Error::Cli { ref stderr, .. } if blink_git::is_lock_error(stderr)));
}
