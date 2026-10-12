use std::fmt::Write as _;
use std::path::Path;

use blink_git::GitCli;

pub struct Fixture {
    pub dir: tempfile::TempDir,
}

impl Fixture {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

pub fn build(files: usize, commits: usize) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    GitCli::init(dir.path(), "main").expect("git init");
    let cli = GitCli::new(dir.path());
    for (k, v) in [("user.name", "Bench"), ("user.email", "bench@example.com"), ("core.autocrlf", "false")] {
        cli.run(&["config", k, v]).expect("config");
    }

    let mut stream = String::new();
    let mut mark = 1;
    let _ =
        writeln!(stream, "commit refs/heads/main\nmark :{mark}\ncommitter Bench <bench@example.com> 1700000000 +0000");
    let _ = writeln!(stream, "data 7\ninitial");
    for i in 0..files {
        let content = format!("file {i}\nline two\nline three\n");
        let _ = writeln!(stream, "M 100644 inline src/d{}/f{i}.txt\ndata {}\n{content}", i % 64, content.len());
    }
    for c in 1..commits {
        let parent = mark;
        mark += 1;
        let message = format!("change {c}");
        let content = format!("revision {c}\n");
        let _ = writeln!(
            stream,
            "commit refs/heads/main\nmark :{mark}\ncommitter Bench <bench@example.com> {} +0000\ndata {}\n{message}\nfrom :{parent}",
            1_700_000_000 + c,
            message.len()
        );
        let _ = writeln!(stream, "M 100644 inline src/hot.txt\ndata {}\n{content}", content.len());
    }
    cli.run_with_stdin(&["fast-import", "--quiet"], stream.as_bytes()).expect("fast-import");
    cli.run(&["checkout", "-f", "main"]).expect("checkout");
    Fixture { dir }
}

pub fn dirty(fixture: &Fixture, every: usize) {
    for i in (0..usize::MAX).step_by(every.max(1)) {
        let path = fixture.path().join(format!("src/d{}/f{i}.txt", i % 64));
        if !path.exists() {
            break;
        }
        std::fs::write(path, format!("file {i}\nCHANGED\nline three\n")).expect("write");
    }
}
