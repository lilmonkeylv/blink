use blink_diff::{FileDiff, LineKind};
use blink_git::{CommitSummary, Edge, GraphRow};
use serde_json::json;
use tauri::ipc::Response;

const VERSION: u8 = 1;
const KIND_DIFF: u8 = 1;
const KIND_HISTORY: u8 = 2;
const KIND_GRAPH: u8 = 3;

struct Writer {
    head: Vec<u8>,
    strings: String,
}

impl Writer {
    fn new() -> Self {
        Self { head: Vec::with_capacity(4096), strings: String::with_capacity(4096) }
    }

    fn u8(&mut self, v: u8) {
        self.head.push(v);
    }

    fn u16(&mut self, v: u16) {
        self.head.extend_from_slice(&v.to_le_bytes());
    }

    fn u32(&mut self, v: u32) {
        self.head.extend_from_slice(&v.to_le_bytes());
    }

    fn f64(&mut self, v: f64) {
        self.head.extend_from_slice(&v.to_le_bytes());
    }

    fn str(&mut self, s: &str) {
        let units = if s.is_ascii() { s.len() } else { s.encode_utf16().count() };
        self.u32(units as u32);
        self.strings.push_str(s);
    }

    fn finish(self, kind: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + self.head.len() + self.strings.len());
        out.push(VERSION);
        out.push(kind);
        out.extend_from_slice(&(self.head.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.head);
        out.extend_from_slice(self.strings.as_bytes());
        out
    }

    fn commits(&mut self, commits: &[CommitSummary]) {
        self.u32(commits.len() as u32);
        for c in commits {
            for s in [&c.id, &c.short_id, &c.summary, &c.author_name, &c.author_email] {
                self.str(s);
            }
            self.f64(c.time as f64);
            self.u16(c.parents.len() as u16);
            for p in &c.parents {
                self.str(p);
            }
        }
    }

    fn edges(&mut self, edges: &[Edge]) {
        self.u16(edges.len() as u16);
        for e in edges {
            self.u16(e.from);
            self.u16(e.to);
            self.u16(e.color);
        }
    }
}

pub fn encode_diff(diff: &FileDiff) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(u8::from(diff.binary) | (u8::from(diff.too_large) << 1));
    w.u32(diff.additions);
    w.u32(diff.deletions);
    let extra = if diff.submodule.is_some() || diff.lfs.is_some() {
        json!({ "submodule": diff.submodule, "lfs": diff.lfs }).to_string()
    } else {
        String::new()
    };
    w.str(&extra);
    w.u32(diff.hunks.len() as u32);
    for hunk in &diff.hunks {
        w.u32(hunk.old_start);
        w.u32(hunk.old_len);
        w.u32(hunk.new_start);
        w.u32(hunk.new_len);
        w.u32(hunk.lines.len() as u32);
        for line in &hunk.lines {
            let kind = match line.kind {
                LineKind::Context => 0,
                LineKind::Added => 1,
                LineKind::Removed => 2,
            };
            w.u8(kind | (u8::from(line.no_newline) << 2) | (u8::from(!line.tokens.is_empty()) << 3));
            w.u32(line.old_no.unwrap_or(0));
            w.u32(line.new_no.unwrap_or(0));
            w.str(&line.text);
            if !line.tokens.is_empty() {
                w.u32(line.tokens.len() as u32);
                for span in &line.tokens {
                    w.u32(span.0);
                    w.u32(span.1);
                    w.u8(span.2);
                }
            }
        }
    }
    w.finish(KIND_DIFF)
}

pub fn encode_history(commits: &[CommitSummary]) -> Vec<u8> {
    let mut w = Writer::new();
    w.commits(commits);
    w.finish(KIND_HISTORY)
}

pub fn encode_graph(commits: &[CommitSummary], rows: &[GraphRow]) -> Vec<u8> {
    let mut w = Writer::new();
    w.commits(commits);
    w.u32(rows.len() as u32);
    for row in rows {
        w.u16(row.lane);
        w.u16(row.color);
        w.u16(row.width);
        w.edges(&row.top);
        w.edges(&row.bottom);
    }
    w.finish(KIND_GRAPH)
}

pub fn diff_response(diff: &FileDiff) -> Response {
    Response::new(encode_diff(diff))
}

#[cfg(test)]
mod tests {
    use super::*;
    use blink_diff::{DiffHunk, DiffLine, LfsDiff, LfsPointer, Span};
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/src/lib/fixtures")
    }

    fn line(kind: LineKind, old_no: Option<u32>, new_no: Option<u32>, text: &str, tokens: Vec<Span>) -> DiffLine {
        DiffLine {
            kind,
            old_no,
            new_no,
            text: text.into(),
            no_newline: false,
            tokens,
            old_raw: Vec::new(),
            new_raw: Vec::new(),
        }
    }

    fn sample_diff() -> FileDiff {
        let mut last = line(LineKind::Added, None, Some(11), "let é = \"🙂\";", vec![Span(4, 1, 8), Span(8, 4, 2)]);
        last.no_newline = true;
        FileDiff {
            additions: 1,
            deletions: 1,
            hunks: vec![DiffHunk {
                old_start: 10,
                old_len: 2,
                new_start: 10,
                new_len: 2,
                lines: vec![
                    line(LineKind::Context, Some(10), Some(10), "fn main() {", vec![Span(0, 2, 5)]),
                    line(LineKind::Removed, Some(11), None, "    old();", vec![]),
                    last,
                ],
            }],
            ..Default::default()
        }
    }

    fn sample_graph() -> (Vec<CommitSummary>, Vec<GraphRow>) {
        let commit = |id: &str, parents: &[&str]| CommitSummary {
            id: id.repeat(40),
            short_id: id.repeat(7),
            summary: format!("Commit {id}: ünïcode"),
            author_name: "Dev".into(),
            author_email: "dev@example.com".into(),
            time: 1_760_000_000,
            parents: parents.iter().map(|p| p.repeat(40)).collect(),
        };
        let commits = vec![commit("b", &["a"]), commit("a", &[])];
        let rows = vec![
            GraphRow { lane: 0, color: 0, top: vec![], bottom: vec![Edge { from: 0, to: 0, color: 0 }], width: 1 },
            GraphRow { lane: 0, color: 0, top: vec![Edge { from: 0, to: 0, color: 0 }], bottom: vec![], width: 1 },
        ];
        (commits, rows)
    }

    fn check_fixture(name: &str, bytes: &[u8]) {
        let path = fixture_dir().join(name);
        if std::env::var_os("BLINK_UPDATE_FIXTURES").is_some() {
            std::fs::create_dir_all(fixture_dir()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        let expected = std::fs::read(&path).expect("run with BLINK_UPDATE_FIXTURES=1 to create the fixture");
        assert_eq!(bytes, expected.as_slice(), "{name} is out of date, rerun with BLINK_UPDATE_FIXTURES=1");
    }

    #[test]
    fn diff_matches_ui_fixture() {
        check_fixture("diff.bin", &encode_diff(&sample_diff()));
    }

    #[test]
    fn graph_matches_ui_fixture() {
        let (commits, rows) = sample_graph();
        check_fixture("graph.bin", &encode_graph(&commits, &rows));
    }

    #[test]
    fn lfs_diff_matches_ui_fixture() {
        let diff = FileDiff {
            lfs: Some(LfsDiff { old: None, new: Some(LfsPointer { oid: "abc".into(), size: 42 }) }),
            ..Default::default()
        };
        check_fixture("lfs.bin", &encode_diff(&diff));
    }

    #[test]
    fn header_length_points_at_string_table() {
        let bytes = encode_history(&sample_graph().0);
        assert_eq!(bytes[..2], [VERSION, KIND_HISTORY]);
        let head = u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize;
        assert!(std::str::from_utf8(&bytes[6 + head..]).unwrap().starts_with(&"b".repeat(40)));
    }
}
