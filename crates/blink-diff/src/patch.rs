use std::collections::HashSet;
use std::fmt::Write;

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
) -> Option<String> {
    let mut body = String::new();
    let mut offset: i64 = 0;

    for (hi, hunk) in diff.hunks.iter().enumerate() {
        let mut lines = String::new();
        let (mut old_len, mut new_len, mut changes) = (0u32, 0u32, 0u32);

        for (li, line) in hunk.lines.iter().enumerate() {
            let selected = selection.contains(hi, li);
            let prefix = match (line.kind, selected, direction) {
                (LineKind::Context, ..) => ' ',
                (LineKind::Added, true, _) => '+',
                (LineKind::Removed, true, _) => '-',
                (LineKind::Added, false, PatchDirection::Stage) => continue,
                (LineKind::Added, false, PatchDirection::Discard) => ' ',
                (LineKind::Removed, false, PatchDirection::Stage) => ' ',
                (LineKind::Removed, false, PatchDirection::Discard) => continue,
            };
            match prefix {
                ' ' => {
                    old_len += 1;
                    new_len += 1;
                }
                '+' => {
                    new_len += 1;
                    changes += 1;
                }
                _ => {
                    old_len += 1;
                    changes += 1;
                }
            }
            lines.push(prefix);
            lines.push_str(&line.text);
            lines.push('\n');
            if line.no_newline {
                lines.push_str("\\ No newline at end of file\n");
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

        let _ = writeln!(body, "@@ -{old_start},{old_len} +{new_start},{new_len} @@");
        body.push_str(&lines);
    }

    if body.is_empty() {
        return None;
    }
    let header = if new_file && direction == PatchDirection::Stage {
        format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n")
    } else {
        format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n")
    };
    Some(header + &body)
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

    #[test]
    fn stage_single_added_line() {
        let diff = diff_bytes(b"a\nb\n", b"a\nx\ny\nb\n", DiffOptions::default());
        let added = diff.hunks[0].lines.iter().position(|l| l.text == "x").unwrap();
        let mut sel = LineSelection::new();
        sel.select(0, added);
        let patch = build_partial_patch("f.txt", &diff, &sel, PatchDirection::Stage, false).unwrap();
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
        let patch = build_partial_patch("f", &diff, &sel, PatchDirection::Discard, false).unwrap();
        assert!(patch.contains(" x\n+y\n"), "{patch}");
        assert!(patch.contains("@@ -1,2 +1,3 @@"), "{patch}");
    }
}
