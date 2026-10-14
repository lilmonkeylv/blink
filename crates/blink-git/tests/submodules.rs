use std::fs;
use std::path::Path;

use blink_git::GitCli;

fn repo(dir: &Path, file: &str) -> GitCli {
    GitCli::init(dir, "main").unwrap();
    let cli = GitCli::new(dir).with_config("protocol.file.allow", "always");
    for (k, v) in [("user.name", "T"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")] {
        cli.run(&["config", k, v]).unwrap();
    }
    fs::write(dir.join(file), "content\n").unwrap();
    cli.stage_all().unwrap();
    cli.commit(&format!("add {file}"), false).unwrap();
    cli
}

fn url(dir: &Path) -> String {
    dir.to_string_lossy().replace('\\', "/")
}

#[test]
fn initializes_updates_and_syncs_submodules() {
    let lib = tempfile::tempdir().unwrap();
    repo(lib.path(), "lib.txt");
    let parent = tempfile::tempdir().unwrap();
    let cli = repo(parent.path(), "app.txt");
    cli.run(&["submodule", "add", &url(lib.path()), "vendor/lib"]).unwrap();
    cli.commit("add lib", false).unwrap();

    let work = tempfile::tempdir().unwrap();
    let clone_dir = work.path().join("clone");
    GitCli::new(work.path()).run(&["clone", &url(parent.path()), &url(&clone_dir)]).unwrap();
    let clone = GitCli::new(&clone_dir).with_config("protocol.file.allow", "always");

    let before = clone.submodules().unwrap();
    assert_eq!(before.len(), 1);
    assert_eq!((before[0].name.as_str(), before[0].path.as_str()), ("vendor/lib", "vendor/lib"));
    assert!(!before[0].initialized);
    assert!(!clone_dir.join("vendor/lib/lib.txt").exists());

    clone.submodule_init().unwrap();
    assert_eq!(clone.config_get("submodule.vendor/lib.url"), Some(url(lib.path())));

    let mut lines = Vec::new();
    clone.submodule_update(&[], |p| lines.push(p.line)).unwrap();
    assert!(clone.submodules().unwrap()[0].initialized);
    assert!(clone_dir.join("vendor/lib/lib.txt").is_file());

    let moved = tempfile::tempdir().unwrap();
    let moved_url = url(moved.path());
    clone.run(&["config", "-f", ".gitmodules", "submodule.vendor/lib.url", &moved_url]).unwrap();
    clone.submodule_sync().unwrap();
    assert_eq!(clone.config_get("submodule.vendor/lib.url"), Some(moved_url.clone()));
    let inner = GitCli::new(clone_dir.join("vendor/lib"));
    assert_eq!(inner.config_get("remote.origin.url"), Some(moved_url));
}

#[test]
fn updates_a_single_submodule() {
    let one = tempfile::tempdir().unwrap();
    repo(one.path(), "one.txt");
    let two = tempfile::tempdir().unwrap();
    repo(two.path(), "two.txt");
    let parent = tempfile::tempdir().unwrap();
    let cli = repo(parent.path(), "app.txt");
    cli.run(&["submodule", "add", &url(one.path()), "one"]).unwrap();
    cli.run(&["submodule", "add", &url(two.path()), "two"]).unwrap();
    cli.commit("add modules", false).unwrap();

    let work = tempfile::tempdir().unwrap();
    let clone_dir = work.path().join("clone");
    GitCli::new(work.path()).run(&["clone", &url(parent.path()), &url(&clone_dir)]).unwrap();
    let clone = GitCli::new(&clone_dir).with_config("protocol.file.allow", "always");
    clone.submodule_update(&["two"], |_| {}).unwrap();
    let state: Vec<_> = clone.submodules().unwrap().into_iter().map(|s| (s.path, s.initialized)).collect();
    assert_eq!(state, [("one".to_string(), false), ("two".to_string(), true)]);
}

#[test]
fn repositories_without_gitmodules_have_no_submodules() {
    let dir = tempfile::tempdir().unwrap();
    let cli = repo(dir.path(), "a.txt");
    assert!(cli.submodules().unwrap().is_empty());
    fs::write(dir.path().join(".gitmodules"), "[submodule \"stale\"]\n\tpath = stale\n\turl = x\n").unwrap();
    assert!(cli.submodules().unwrap().is_empty());
}
