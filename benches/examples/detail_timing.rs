use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("repo path");
    let repo = blink_git::Repository::open(&path).unwrap();
    let revs: Vec<String> = match args.next() {
        Some(rev) => vec![rev],
        None => repo.log(None, 0, 10).unwrap().into_iter().map(|c| c.id).collect(),
    };
    for rev in revs {
        let t = Instant::now();
        match repo.commit_detail(&rev) {
            Ok(d) => println!("{} files={} {:?}", d.commit.short_id, d.files.len(), t.elapsed()),
            Err(e) => println!("{rev} ERROR {e} {:?}", t.elapsed()),
        }
    }
}
