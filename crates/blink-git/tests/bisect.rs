use std::fs;

use blink_git::{BisectMark, Error, GitCli, Repository, is_bisecting};

const BROKEN_AT: usize = 5;

fn fixture() -> (tempfile::TempDir, Repository, Vec<String>) {
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
    for i in 0..10 {
        fs::write(dir.path().join("status"), if i >= BROKEN_AT { "broken\n" } else { "fine\n" }).unwrap();
        fs::write(dir.path().join("counter"), format!("{i}\n")).unwrap();
        cli.stage_all().unwrap();
        ids.push(cli.commit(&format!("change {i}"), false).unwrap());
    }
    let repo = Repository::open(dir.path()).unwrap();
    (dir, repo, ids)
}

#[test]
fn bisect_finds_the_first_bad_commit() {
    let (dir, repo, ids) = fixture();
    let cli = repo.cli();
    assert_eq!(cli.bisect_state(repo.git_dir()).unwrap(), None);

    cli.bisect_start(&ids[9], None).unwrap();
    assert!(is_bisecting(repo.git_dir()));
    let waiting = cli.bisect_state(repo.git_dir()).unwrap().unwrap();
    assert_eq!(waiting.bad.as_deref(), Some(ids[9].as_str()));
    assert!(waiting.good.is_empty());
    assert_eq!((waiting.revisions_left, waiting.first_bad.as_deref()), (None, None));

    cli.bisect_mark(BisectMark::Good, Some(&ids[0])).unwrap();
    let mut state = cli.bisect_state(repo.git_dir()).unwrap().unwrap();
    assert_eq!(state.good, [ids[0].clone()]);
    assert!(state.steps_left.is_some_and(|s| s >= 2), "{state:?}");
    let mut rounds = 0;
    while state.first_bad.is_none() {
        rounds += 1;
        assert!(rounds < 10, "bisect did not converge: {state:?}");
        let current = state.current.clone().unwrap();
        assert!(state.current_summary.as_deref().is_some_and(|s| s.starts_with("change ")));
        let broken = fs::read_to_string(dir.path().join("status")).unwrap() == "broken\n";
        let mark = if broken { BisectMark::Bad } else { BisectMark::Good };
        cli.bisect_mark(mark, None).unwrap();
        state = cli.bisect_state(repo.git_dir()).unwrap().unwrap();
        assert!(state.good.contains(&current) || state.bad.as_deref() == Some(current.as_str()));
    }
    assert_eq!(state.first_bad.as_deref(), Some(ids[BROKEN_AT].as_str()));
    assert_eq!(state.first_bad_summary.as_deref(), Some("change 5"));

    cli.bisect_reset().unwrap();
    assert!(!is_bisecting(repo.git_dir()));
    assert_eq!(repo.head().unwrap().branch_name(), Some("main"));
}

#[test]
fn skipped_commits_are_recorded() {
    let (_dir, repo, ids) = fixture();
    let cli = repo.cli();
    cli.bisect_start(&ids[9], Some(&ids[0])).unwrap();
    let before = cli.bisect_state(repo.git_dir()).unwrap().unwrap();
    let current = before.current.clone().unwrap();
    assert_ne!(current, ids[9]);
    cli.bisect_mark(BisectMark::Skip, None).unwrap();
    let after = cli.bisect_state(repo.git_dir()).unwrap().unwrap();
    assert_eq!(after.skipped, [current.as_str()]);
    assert_ne!(after.current.as_deref(), Some(current.as_str()));
    cli.bisect_reset().unwrap();
}

#[test]
fn bisect_rejects_anything_but_commit_ids() {
    let (_dir, repo, ids) = fixture();
    let cli = repo.cli();
    assert!(matches!(cli.bisect_start("HEAD", None), Err(Error::InvalidArgument(_))));
    assert!(matches!(cli.bisect_start(&ids[9], Some("--term-bad=x")), Err(Error::InvalidArgument(_))));
    assert!(matches!(cli.bisect_mark(BisectMark::Good, Some("main")), Err(Error::InvalidArgument(_))));
    assert!(!is_bisecting(repo.git_dir()));
}
