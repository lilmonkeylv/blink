use std::time::Instant;

use blink_diff::{DiffOptions, diff_file};

fn main() {
    let old: String =
        (0..10_000).map(|i| format!("fn f{i}(x: u32) -> u32 {{ let s = \"v{i}\"; x + {i} }}\n")).collect();
    let near_top = old.replacen("fn f5(", "fn g5(", 1);
    let near_end = old.replacen("fn f9990(", "fn g9990(", 1);
    blink_diff::warm_up();
    for (label, new) in [("change near top", &near_top), ("change near end", &near_end)] {
        let t = Instant::now();
        diff_file("x.rs", old.as_bytes(), new.as_bytes(), DiffOptions { highlight: false, ..Default::default() });
        let plain = t.elapsed();
        let t = Instant::now();
        diff_file("x.rs", old.as_bytes(), new.as_bytes(), DiffOptions::default());
        println!("{label}: plain {plain:?}, highlighted {:?}", t.elapsed());
    }
}
