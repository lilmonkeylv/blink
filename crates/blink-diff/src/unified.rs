use crate::{DiffHunk, DiffLine, FileDiff, LineKind};

fn parse_range(spec: &str) -> Option<(u32, u32)> {
    let spec = spec.get(1..)?;
    match spec.split_once(',') {
        Some((start, len)) => Some((start.parse().ok()?, len.parse().ok()?)),
        None => Some((spec.parse().ok()?, 1)),
    }
}

fn parse_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = line.strip_prefix("@@ ")?.split(' ');
    let (old_start, old_len) = parse_range(parts.next()?)?;
    let (new_start, new_len) = parse_range(parts.next()?)?;
    Some((old_start, old_len, new_start, new_len))
}

pub fn parse_unified(patch: &str) -> FileDiff {
    let mut diff = FileDiff::default();
    let (mut old_no, mut new_no) = (0u32, 0u32);
    for line in patch.lines() {
        if let Some((old_start, old_len, new_start, new_len)) = parse_header(line) {
            diff.hunks.push(DiffHunk { old_start, old_len, new_start, new_len, lines: Vec::new() });
            (old_no, new_no) = (old_start, new_start);
            continue;
        }
        let Some(hunk) = diff.hunks.last_mut() else { continue };
        if line.starts_with('\\') {
            if let Some(last) = hunk.lines.last_mut() {
                last.no_newline = true;
            }
            continue;
        }
        let (kind, text) = match line.chars().next() {
            Some('+') => (LineKind::Added, &line[1..]),
            Some('-') => (LineKind::Removed, &line[1..]),
            Some(' ') => (LineKind::Context, &line[1..]),
            None => (LineKind::Context, ""),
            _ => continue,
        };
        let (old, new) = match kind {
            LineKind::Added => {
                diff.additions += 1;
                new_no += 1;
                (None, Some(new_no - 1))
            }
            LineKind::Removed => {
                diff.deletions += 1;
                old_no += 1;
                (Some(old_no - 1), None)
            }
            LineKind::Context => {
                old_no += 1;
                new_no += 1;
                (Some(old_no - 1), Some(new_no - 1))
            }
        };
        hunk.lines.push(DiffLine {
            kind,
            old_no: old,
            new_no: new,
            text: text.to_string(),
            no_newline: false,
            tokens: Vec::new(),
            old_raw: Vec::new(),
            new_raw: Vec::new(),
        });
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_style_patches() {
        let patch = "@@ -1,3 +1,4 @@\n line one\n-old two\n+new two\n+added\n line three\n@@ -10 +11 @@ fn tail()\n-x\n\\ No newline at end of file\n+y";
        let diff = parse_unified(patch);
        assert_eq!((diff.additions, diff.deletions), (3, 2));
        assert_eq!(diff.hunks.len(), 2);
        let first = &diff.hunks[0];
        assert_eq!((first.old_start, first.old_len, first.new_start, first.new_len), (1, 3, 1, 4));
        assert_eq!(first.lines[1].old_no, Some(2));
        assert_eq!(first.lines[3].new_no, Some(3));
        assert_eq!(first.lines[4].old_no, Some(3));
        assert_eq!(first.lines[4].new_no, Some(4));
        let second = &diff.hunks[1];
        assert_eq!((second.old_start, second.old_len, second.new_len), (10, 1, 1));
        assert!(second.lines[0].no_newline);
        assert_eq!(second.lines[1].new_no, Some(11));
    }

    #[test]
    fn empty_patch_has_no_hunks() {
        assert!(parse_unified("").hunks.is_empty());
    }
}
