use std::fs;

use blink_git::{GitCli, uses_lfs};

fn lfs_repo() -> Option<(tempfile::TempDir, GitCli)> {
    let dir = tempfile::tempdir().unwrap();
    GitCli::init(dir.path(), "main").unwrap();
    let cli = GitCli::new(dir.path());
    if cli.lfs_version().is_none() {
        eprintln!("git-lfs is not installed, skipping");
        return None;
    }
    Some((dir, cli))
}

#[test]
fn tracks_patterns_in_gitattributes() {
    let Some((dir, cli)) = lfs_repo() else { return };
    assert!(cli.lfs_version().unwrap().starts_with("git-lfs/"));
    cli.lfs_track("*.psd").unwrap();
    cli.lfs_track("art/big file.bin").unwrap();
    let attributes = fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert!(attributes.contains("*.psd filter=lfs diff=lfs merge=lfs -text"), "{attributes}");
    assert!(attributes.contains("art/big[[:space:]]file.bin filter=lfs"), "{attributes}");
    assert!(uses_lfs(&attributes));

    cli.lfs_track("*.psd").unwrap();
    let again = fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert_eq!(again.matches("*.psd").count(), 1);
    assert!(cli.lfs_track("# not a pattern").is_err());
    assert!(cli.lfs_track("a\nb").is_err());
}

#[test]
fn a_repository_without_a_remote_has_no_locks() {
    let Some((_dir, cli)) = lfs_repo() else { return };
    assert!(cli.lfs_locks().unwrap().is_empty());
}
