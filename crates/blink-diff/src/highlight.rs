use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use serde::Serialize;
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

const MAX_LINES: usize = 20_000;
const MAX_LINE_LEN: usize = 2_000;

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span(pub u32, pub u32, pub u8);

const CLASSES: &[(&str, u8)] = &[
    ("comment", 1),
    ("string", 2),
    ("constant.numeric", 3),
    ("constant.character.escape", 2),
    ("constant", 4),
    ("entity.name.function", 6),
    ("support.function", 6),
    ("meta.function-call", 6),
    ("entity.name.type", 7),
    ("entity.name.class", 7),
    ("support.type", 7),
    ("support.class", 7),
    ("storage.type", 5),
    ("storage", 5),
    ("keyword.operator", 0),
    ("keyword", 5),
    ("variable.parameter", 8),
    ("variable.language", 4),
    ("entity.name.tag", 9),
    ("entity.other.attribute-name", 10),
    ("markup.heading", 6),
    ("markup.bold", 5),
    ("markup.underline.link", 2),
];

thread_local! {
    static SCOPE_CLASS: RefCell<HashMap<Scope, u8>> = RefCell::new(HashMap::new());
}

fn scope_class(scope: Scope) -> u8 {
    SCOPE_CLASS.with(|cache| {
        *cache.borrow_mut().entry(scope).or_insert_with(|| {
            let name = scope.build_string();
            CLASSES.iter().find(|(prefix, _)| name.starts_with(prefix)).map(|(_, c)| *c).unwrap_or(0)
        })
    })
}

fn stack_class(stack: &ScopeStack) -> u8 {
    stack.as_slice().iter().rev().map(|s| scope_class(*s)).find(|c| *c != 0).unwrap_or(0)
}

pub fn warm_up() {
    LazyLock::force(&SYNTAXES);
}

pub fn syntax_for(path: &str, first_line: &str) -> Option<&'static SyntaxReference> {
    let ss = &*SYNTAXES;
    let p = Path::new(path);
    p.extension()
        .and_then(|e| ss.find_syntax_by_extension(&e.to_string_lossy()))
        .or_else(|| p.file_name().and_then(|n| ss.find_syntax_by_extension(&n.to_string_lossy())))
        .or_else(|| ss.find_syntax_by_first_line(first_line))
        .filter(|s| s.name != "Plain Text")
}

pub fn highlight(path: &str, text: &str, until: usize) -> Option<Vec<Vec<Span>>> {
    let syntax = syntax_for(path, text.lines().next().unwrap_or_default())?;
    let ss = &*SYNTAXES;
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut out = Vec::new();

    for (index, raw) in text.split_inclusive('\n').enumerate() {
        if index >= MAX_LINES.min(until) {
            break;
        }
        if raw.len() > MAX_LINE_LEN {
            out.push(Vec::new());
            continue;
        }
        let ops = state.parse_line(raw, ss).ok()?;
        let visible = raw.trim_end_matches(['\n', '\r']).len();
        let mut spans: Vec<Span> = Vec::new();
        let mut pos = 0usize;
        let emit = |start: usize, end: usize, class: u8, spans: &mut Vec<Span>| {
            let end = end.min(visible);
            if class == 0 || start >= end {
                return;
            }
            let start16 = utf16_len(&raw[..start]);
            let len16 = utf16_len(&raw[start..end]);
            match spans.last_mut() {
                Some(last) if last.2 == class && last.0 + last.1 == start16 => last.1 += len16,
                _ => spans.push(Span(start16, len16, class)),
            }
        };
        for (offset, op) in ops {
            emit(pos, offset, stack_class(&stack), &mut spans);
            let _ = stack.apply(&op);
            pos = offset;
        }
        emit(pos, raw.len(), stack_class(&stack), &mut spans);
        out.push(spans);
    }
    Some(out)
}

fn utf16_len(s: &str) -> u32 {
    s.encode_utf16().count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_keywords_and_strings() {
        let lines = highlight("main.rs", "fn main() {\n    let s = \"hi\"; // note\n}\n", usize::MAX).unwrap();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].iter().any(|s| s.2 == 5 && s.0 == 0 && s.1 == 2));
        assert!(lines[1].iter().any(|s| s.2 == 2));
        assert!(lines[1].iter().any(|s| s.2 == 1));
    }

    #[test]
    fn unknown_extension_is_not_highlighted() {
        assert!(highlight("notes.unknownext", "hello\n", usize::MAX).is_none());
    }

    #[test]
    fn offsets_are_utf16() {
        let lines = highlight("a.js", "const é = \"x\";\n", usize::MAX).unwrap();
        let string = lines[0].iter().find(|s| s.2 == 2).unwrap();
        assert_eq!(string.0, 10);
    }
}
