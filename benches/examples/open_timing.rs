use std::time::Instant;

fn main() {
    let files: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(50_000);
    let fixture = blink_benches::build(files, 2_000);
    let t = Instant::now();
    let repo = blink_git::Repository::open(fixture.path()).unwrap();
    println!("open        {:?}", t.elapsed());
    let t = Instant::now();
    let status = repo.status(None).unwrap();
    println!("status      {:?} ({} files)", t.elapsed(), status.files.len());
    let t = Instant::now();
    let _ = repo.head().unwrap();
    let _ = repo.current_ahead_behind().unwrap();
    println!("head+ab     {:?}", t.elapsed());
    let t = Instant::now();
    let _ = repo.branches().unwrap();
    println!("branches    {:?}", t.elapsed());
    let (tx, _rx) = crossbeam_channel::unbounded();
    let t = Instant::now();
    let _w = blink_watch::RepoWatcher::start(&repo, blink_watch::DEFAULT_DEBOUNCE, tx).unwrap();
    println!("watcher     {:?}", t.elapsed());
    let t = Instant::now();
    let _ = repo.log(None, 0, 300).unwrap();
    println!("log 300     {:?}", t.elapsed());
}
