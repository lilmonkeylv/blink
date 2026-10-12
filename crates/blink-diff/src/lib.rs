mod highlight;
mod patch;

use imara_diff::{Algorithm, Diff, InternedInput};
use serde::Serialize;

pub use highlight::{Span, highlight, warm_up};
pub use patch::{LineSelection, PatchDirection, build_partial_patch};

pub const DEFAULT_CONTEXT: u32 = 3;
pub const LARGE_DIFF_LINES: usize = 5_000;
const BINARY_SNIFF_LEN: usize = 8_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
    pub no_newline: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    pub fn header(&self) -> String {
        format!("@@ -{} +{} @@", range_label(self.old_start, self.old_len), range_label(self.new_start, self.new_len))
    }
}

fn range_label(start: u32, len: u32) -> String {
    if len == 1 { start.to_string() } else { format!("{start},{len}") }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub binary: bool,
    pub too_large: bool,
    pub additions: u32,
    pub deletions: u32,
    pub hunks: Vec<DiffHunk>,
}

impl FileDiff {
    pub fn line_count(&self) -> usize {
        self.hunks.iter().map(|h| h.lines.len()).sum()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiffOptions {
    pub context: u32,
    pub ignore_whitespace: bool,
    pub max_lines: Option<usize>,
    pub highlight: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self { context: DEFAULT_CONTEXT, ignore_whitespace: false, max_lines: Some(LARGE_DIFF_LINES), highlight: true }
    }
}

pub fn is_binary(data: &[u8]) -> bool {
    data[..data.len().min(BINARY_SNIFF_LEN)].contains(&0)
}

struct SplitLines<'a> {
    lines: Vec<&'a [u8]>,
    missing_final_newline: bool,
}

fn split_lines(data: &[u8]) -> SplitLines<'_> {
    let mut lines: Vec<&[u8]> = data.split_inclusive(|&b| b == b'\n').collect();
    let missing_final_newline = lines.last().is_some_and(|l| !l.ends_with(b"\n"));
    for line in &mut lines {
        let mut end = line.len();
        if end > 0 && line[end - 1] == b'\n' {
            end -= 1;
            if end > 0 && line[end - 1] == b'\r' {
                end -= 1;
            }
        }
        *line = &line[..end];
    }
    SplitLines { lines, missing_final_newline }
}

fn normalize_whitespace(line: &[u8]) -> Vec<u8> {
    line.split(|b| b.is_ascii_whitespace()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(&b' ')
}

pub fn diff_bytes(old: &[u8], new: &[u8], opts: DiffOptions) -> FileDiff {
    if is_binary(old) || is_binary(new) {
        return FileDiff { binary: old != new, ..Default::default() };
    }
    let before = split_lines(old);
    let after = split_lines(new);

    let diff = if opts.ignore_whitespace {
        let b: Vec<Vec<u8>> = before.lines.iter().map(|l| normalize_whitespace(l)).collect();
        let a: Vec<Vec<u8>> = after.lines.iter().map(|l| normalize_whitespace(l)).collect();
        compute(&b, &a)
    } else {
        compute(&before.lines, &after.lines)
    };

    let mut out =
        FileDiff { additions: diff.count_additions(), deletions: diff.count_removals(), ..Default::default() };
    if let Some(max) = opts.max_lines
        && (out.additions + out.deletions) as usize > max
    {
        out.too_large = true;
        return out;
    }
    out.hunks = build_hunks(&diff, &before, &after, opts.context);
    out
}

pub fn diff_file(path: &str, old: &[u8], new: &[u8], opts: DiffOptions) -> FileDiff {
    let mut diff = diff_bytes(old, new, opts);
    if diff.hunks.is_empty() || !opts.highlight {
        return diff;
    }
    let lines = diff.hunks.iter().flat_map(|h| &h.lines);
    let last_old = lines.clone().filter(|l| l.kind == LineKind::Removed).filter_map(|l| l.old_no).max().unwrap_or(0);
    let last_new = lines.filter_map(|l| l.new_no).max().unwrap_or(0);
    let (old_tokens, new_tokens) = std::thread::scope(|scope| {
        let old_job =
            (last_old > 0).then(|| scope.spawn(|| highlight(path, &String::from_utf8_lossy(old), last_old as usize)));
        let new_tokens = highlight(path, &String::from_utf8_lossy(new), last_new as usize);
        (old_job.and_then(|j| j.join().ok().flatten()), new_tokens)
    });
    let pick = |tokens: &Option<Vec<Vec<Span>>>, no: Option<u32>| {
        tokens.as_ref().zip(no).and_then(|(t, n)| t.get(n as usize - 1).cloned()).unwrap_or_default()
    };
    for hunk in &mut diff.hunks {
        for line in &mut hunk.lines {
            line.tokens = match line.kind {
                LineKind::Removed => pick(&old_tokens, line.old_no),
                LineKind::Added | LineKind::Context => pick(&new_tokens, line.new_no),
            };
        }
    }
    diff
}

fn compute<T: AsRef<[u8]> + Eq + std::hash::Hash>(before: &[T], after: &[T]) -> Diff {
    let input = InternedInput::new(LineTokens(before), LineTokens(after));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_no_heuristic(&input);
    diff
}

struct LineTokens<'a, T>(&'a [T]);

impl<'a, T: Eq + std::hash::Hash> imara_diff::TokenSource for LineTokens<'a, T> {
    type Token = &'a T;
    type Tokenizer = std::slice::Iter<'a, T>;

    fn tokenize(&self) -> Self::Tokenizer {
        self.0.iter()
    }

    fn estimate_tokens(&self) -> u32 {
        self.0.len() as u32
    }
}

fn text(line: &[u8]) -> String {
    String::from_utf8_lossy(line).into_owned()
}

fn build_hunks(diff: &Diff, before: &SplitLines<'_>, after: &SplitLines<'_>, context: u32) -> Vec<DiffHunk> {
    let raw: Vec<imara_diff::Hunk> = diff.hunks().collect();
    if raw.is_empty() {
        return Vec::new();
    }
    let old_total = before.lines.len() as u32;
    let new_total = after.lines.len() as u32;

    let mut groups: Vec<Vec<imara_diff::Hunk>> = Vec::new();
    for hunk in raw {
        match groups.last_mut() {
            Some(group) if hunk.before.start - group.last().unwrap().before.end <= context * 2 => group.push(hunk),
            _ => groups.push(vec![hunk]),
        }
    }

    let push_context = |lines: &mut Vec<DiffLine>, old_idx: u32, new_idx: u32| {
        lines.push(DiffLine {
            kind: LineKind::Context,
            old_no: Some(old_idx + 1),
            new_no: Some(new_idx + 1),
            text: text(before.lines[old_idx as usize]),
            no_newline: old_idx + 1 == old_total && before.missing_final_newline,
            tokens: Vec::new(),
        });
    };

    groups
        .into_iter()
        .map(|group| {
            let first = &group[0];
            let last = group.last().unwrap();
            let old_start = first.before.start.saturating_sub(context);
            let new_start = first.after.start - (first.before.start - old_start);
            let old_end = (last.before.end + context).min(old_total);
            let new_end = (last.after.end + (old_end - last.before.end)).min(new_total);

            let mut lines = Vec::new();
            let (mut o, mut n) = (old_start, new_start);
            for h in &group {
                while o < h.before.start {
                    push_context(&mut lines, o, n);
                    o += 1;
                    n += 1;
                }
                for i in h.before.clone() {
                    lines.push(DiffLine {
                        kind: LineKind::Removed,
                        old_no: Some(i + 1),
                        new_no: None,
                        text: text(before.lines[i as usize]),
                        no_newline: i + 1 == old_total && before.missing_final_newline,
                        tokens: Vec::new(),
                    });
                }
                for i in h.after.clone() {
                    lines.push(DiffLine {
                        kind: LineKind::Added,
                        old_no: None,
                        new_no: Some(i + 1),
                        text: text(after.lines[i as usize]),
                        no_newline: i + 1 == new_total && after.missing_final_newline,
                        tokens: Vec::new(),
                    });
                }
                o = h.before.end;
                n = h.after.end;
            }
            while o < old_end && n < new_end {
                push_context(&mut lines, o, n);
                o += 1;
                n += 1;
            }

            let old_len = old_end - old_start;
            let new_len = new_end - new_start;
            DiffHunk {
                old_start: if old_len == 0 { old_start } else { old_start + 1 },
                old_len,
                new_start: if new_len == 0 { new_start } else { new_start + 1 },
                new_len,
                lines,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(a: &str, b: &str) -> FileDiff {
        diff_bytes(a.as_bytes(), b.as_bytes(), DiffOptions::default())
    }

    #[test]
    fn identical_has_no_hunks() {
        assert!(d("a\nb\n", "a\nb\n").hunks.is_empty());
    }

    #[test]
    fn single_modification() {
        let diff = d("a\nb\nc\n", "a\nB\nc\n");
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 1);
        assert_eq!(diff.hunks.len(), 1);
        let h = &diff.hunks[0];
        assert_eq!(h.header(), "@@ -1,3 +1,3 @@");
        let kinds: Vec<_> = h.lines.iter().map(|l| l.kind).collect();
        assert_eq!(kinds, [LineKind::Context, LineKind::Removed, LineKind::Added, LineKind::Context]);
    }

    #[test]
    fn new_file() {
        let diff = d("", "x\ny\n");
        assert_eq!(diff.hunks[0].header(), "@@ -0,0 +1,2 @@");
    }

    #[test]
    fn distant_changes_split_into_hunks() {
        let old: String = (0..30).map(|i| format!("<{i}>\n")).collect();
        let new = old.replace("<2>", "two").replace("<25>", "twentyfive");
        assert_eq!(d(&old, &new).hunks.len(), 2);
    }

    #[test]
    fn binary_detected() {
        let diff = diff_bytes(b"a\0b", b"a\0c", DiffOptions::default());
        assert!(diff.binary);
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn missing_newline_flagged() {
        let diff = d("a\nb", "a\nc");
        let last = diff.hunks[0].lines.last().unwrap();
        assert!(last.no_newline);
    }

    #[test]
    fn ignore_whitespace() {
        let opts = DiffOptions { ignore_whitespace: true, ..Default::default() };
        assert!(diff_bytes(b"a  b\n", b"a b\n", opts).hunks.is_empty());
    }

    #[test]
    fn diff_file_attaches_tokens() {
        let diff = diff_file(
            "x.rs",
            b"fn a() {}
",
            b"fn b() {}
",
            DiffOptions::default(),
        );
        assert!(diff.hunks[0].lines.iter().all(|l| !l.tokens.is_empty()));
    }

    #[test]
    fn crlf_is_stripped_from_text() {
        let diff = d("a\r\nb\r\n", "a\r\nc\r\n");
        assert!(diff.hunks[0].lines.iter().all(|l| !l.text.ends_with('\r')));
    }
}
