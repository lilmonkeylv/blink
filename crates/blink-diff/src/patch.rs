use std::collections::HashSet;

use crate::{FileDiff, LineKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchDirection {
    Stage,
    Discard,
}

#[derive(Debug, Clone, Default)]
pub struct LineSelection {
    selected: HashSet<(usize, usize)>,
}

impl LineSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn select(&mut self, hunk: usize, line: usize) -> &mut Self {
        self.selected.insert((hunk, line));
        self
    }

    pub fn select_hunk(&mut self, diff: &FileDiff, hunk: usize) -> &mut Self {
        if let Some(h) = diff.hunks.get(hunk) {
            for (i, l) in h.lines.iter().enumerate() {
                if l.kind != LineKind::Context {
                    self.selected.insert((hunk, i));
                }
            }
        }
        self
    }

    pub fn contains(&self, hunk: usize, line: usize) -> bool {
        self.selected.contains(&(hunk, line))
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
}

pub fn build_partial_patch(
    path: &str,
    diff: &FileDiff,
    selection: &LineSelection,
    direction: PatchDirection,
    new_file: bool,
) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut offset: i64 = 0;

    for (hi, hunk) in diff.hunks.iter().enumerate() {
        let mut lines = Vec::new();
        let (mut old_len, mut new_len, mut changes) = (0u32, 0u32, 0u32);

        for (li, line) in hunk.lines.iter().enumerate() {
            let selected = selection.contains(hi, li);
            let prefix = match (line.kind, selected, direction) {
                (LineKind::Context, ..) => b' ',
                (LineKind::Added, true, _) => b'+',
                (LineKind::Removed, true, _) => b'-',
                (LineKind::Added, false, PatchDirection::Stage) => continue,
                (LineKind::Added, false, PatchDirection::Discard) => b' ',
                (LineKind::Removed, false, PatchDirection::Stage) => b' ',
                (LineKind::Removed, false, PatchDirection::Discard) => continue,
            };
            let raw = match (prefix, direction) {
                (b'+', _) | (b' ', PatchDirection::Discard) => &line.new_raw,
                _ => &line.old_raw,
            };
            match prefix {
                b' ' => {
                    old_len += 1;
                    new_len += 1;
                }
                b'+' => {
                    new_len += 1;
                    changes += 1;
                }
                _ => {
                    old_len += 1;
                    changes += 1;
                }
            }
            lines.push(prefix);
            lines.extend_from_slice(raw);
            if !raw.ends_with(b"\n") {
                lines.extend_from_slice(b"\n\\ No newline at end of file\n");
            }
        }

        if changes == 0 {
            continue;
        }
        let (old_start, new_start) = match direction {
            PatchDirection::Stage => {
                let old_start = hunk.old_start;
                (old_start, counterpart_start(old_start, old_len, new_len, offset))
            }
            PatchDirection::Discard => {
                let new_start = hunk.new_start;
                (counterpart_start(new_start, new_len, old_len, -offset), new_start)
            }
        };
        offset += new_len as i64 - old_len as i64;

        body.extend_from_slice(format!("@@ -{old_start},{old_len} +{new_start},{new_len} @@\n").as_bytes());
        body.append(&mut lines);
    }

    if body.is_empty() {
        return None;
    }
    let header = if new_file && direction == PatchDirection::Stage {
        format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n")
    } else {
        format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n")
    };
    let mut patch = header.into_bytes();
    patch.append(&mut body);
    Some(patch)
}

fn counterpart_start(anchor: u32, anchor_len: u32, other_len: u32, offset: i64) -> u32 {
    let base = anchor as i64 + offset;
    let start = match (anchor_len == 0, other_len == 0) {
        (true, false) => base + 1,
        (false, true) => base - 1,
        _ => base,
    };
    start.max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffOptions, diff_bytes};

    fn patch_text(patch: Vec<u8>) -> String {
        String::from_utf8(patch).unwrap()
    }

    #[test]
    fn stage_single_added_line() {
        let diff = diff_bytes(b"a\nb\n", b"a\nx\ny\nb\n", DiffOptions::default());
        let added = diff.hunks[0].lines.iter().position(|l| l.text == "x").unwrap();
        let mut sel = LineSelection::new();
        sel.select(0, added);
        let patch = patch_text(build_partial_patch("f.txt", &diff, &sel, PatchDirection::Stage, false).unwrap());
        assert!(patch.contains("@@ -1,2 +1,3 @@"), "{patch}");
        assert!(patch.contains("+x\n"));
        assert!(!patch.contains("y"));
    }

    #[test]
    fn unselected_removal_becomes_context_when_staging() {
        let diff = diff_bytes(b"a\nb\nc\n", b"a\nc\n", DiffOptions::default());
        let mut sel = LineSelection::new();
        sel.select(0, 0);
        assert!(build_partial_patch("f", &diff, &sel, PatchDirection::Stage, false).is_none());
    }

    #[test]
    fn discard_keeps_unselected_additions_as_context() {
        let diff = diff_bytes(b"a\n", b"a\nx\ny\n", DiffOptions::default());
        let y = diff.hunks[0].lines.iter().position(|l| l.text == "y").unwrap();
        let mut sel = LineSelection::new();
        sel.select(0, y);
        let patch = patch_text(build_partial_patch("f", &diff, &sel, PatchDirection::Discard, false).unwrap());
        assert!(patch.contains(" x\n+y\n"), "{patch}");
        assert!(patch.contains("@@ -1,2 +1,3 @@"), "{patch}");
    }

    #[test]
    fn keeps_original_line_endings_and_bytes() {
        let diff = diff_bytes(b"a\r\nb\r\n", b"a\r\n\xe9t\xe9\r\nb\r\n", DiffOptions::default());
        let added = diff.hunks[0].lines.iter().position(|l| l.kind == LineKind::Added).unwrap();
        let mut sel = LineSelection::new();
        sel.select(0, added);
        let patch = build_partial_patch("f", &diff, &sel, PatchDirection::Stage, false).unwrap();
        let body = &patch[patch.windows(2).position(|w| w == b"@@").unwrap()..];
        assert_eq!(body, b"@@ -1,2 +1,3 @@\n a\r\n+\xe9t\xe9\r\n b\r\n");
    }

    #[test]
    fn marks_missing_final_newline_from_the_raw_line() {
        let diff = diff_bytes(b"a\nb", b"a\nc", DiffOptions::default());
        let mut sel = LineSelection::new();
        sel.select_hunk(&diff, 0);
        let patch = patch_text(build_partial_patch("f", &diff, &sel, PatchDirection::Stage, false).unwrap());
        assert!(patch.ends_with("-b\n\\ No newline at end of file\n+c\n\\ No newline at end of file\n"), "{patch}");
    }
}
