use std::time::Duration;

use blink_core::{Engine, PoolConfig};
use blink_diff::{DiffOptions, diff_bytes};
use blink_git::Repository;
use criterion::{Criterion, criterion_group, criterion_main};

fn status(c: &mut Criterion) {
    let mut group = c.benchmark_group("status");
    group.sample_size(20).measurement_time(Duration::from_secs(8));
    for files in [5_000usize, 20_000] {
        let fixture = blink_benches::build(files, 1);
        let repo = Repository::open(fixture.path()).unwrap();
        repo.status(None).unwrap();
        group.bench_function(format!("clean_{files}"), |b| b.iter(|| repo.status(None).unwrap()));
        blink_benches::dirty(&fixture, 50);
        repo.status(None).unwrap();
        group.bench_function(format!("dirty_{files}"), |b| b.iter(|| repo.status(None).unwrap()));
    }
    group.finish();
}

fn history(c: &mut Criterion) {
    let fixture = blink_benches::build(10, 20_000);
    let repo = Repository::open(fixture.path()).unwrap();
    let mut group = c.benchmark_group("history");
    group.bench_function("first_page_200", |b| b.iter(|| repo.log(None, 0, 200).unwrap()));
    group.bench_function("commit_detail", |b| b.iter(|| repo.commit_detail("HEAD").unwrap()));
    group.bench_function("branches", |b| b.iter(|| repo.branches().unwrap()));
    group.finish();
}

fn diff(c: &mut Criterion) {
    let old: String = (0..10_000).map(|i| format!("let value_{i} = compute({i});\n")).collect();
    let new: String = old
        .lines()
        .enumerate()
        .map(|(i, l)| if i % 37 == 0 { format!("{l} // changed\n") } else { format!("{l}\n") })
        .collect();
    let mut group = c.benchmark_group("diff");
    group.bench_function("10k_lines", |b| {
        b.iter(|| diff_bytes(old.as_bytes(), new.as_bytes(), DiffOptions { max_lines: None, ..Default::default() }))
    });
    group.finish();
}

fn engine_refresh(c: &mut Criterion) {
    let fixture = blink_benches::build(5_000, 50);
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let engine = Engine::new(PoolConfig::default());
    let (id, _) = runtime.block_on(engine.open(fixture.path())).unwrap();
    c.bench_function("engine/refresh_5000", |b| b.iter(|| runtime.block_on(engine.refresh(&id)).unwrap()));
}

criterion_group!(benches, status, history, diff, engine_refresh);
criterion_main!(benches);
