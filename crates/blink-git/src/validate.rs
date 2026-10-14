use std::path::{Component, Path};

use crate::error::{Error, Result};

pub fn safe_rel_path(path: &str) -> Result<&Path> {
    let candidate = Path::new(path);
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let escapes = path.split(['/', '\\']).any(|segment| segment == "..");
    let plain = candidate.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if path.trim().is_empty() || path.contains('\0') || path.starts_with(['/', '\\']) || drive || escapes || !plain {
        return Err(Error::InvalidArgument(format!("path {path:?} is outside the repository")));
    }
    Ok(candidate)
}

pub fn validate_sha(sha: &str) -> Result<&str> {
    if (4..=64).contains(&sha.len()) && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(sha)
    } else {
        Err(Error::InvalidArgument(format!("{sha:?} is not a commit id")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_paths_inside_the_worktree() {
        for ok in ["src/main.rs", "a", "./a/b", "dir\\file.txt", "name..with..dots", "a/.hidden"] {
            assert!(safe_rel_path(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn rejects_paths_that_escape_the_worktree() {
        for bad in [
            "",
            "/etc/passwd",
            "\\Windows\\win.ini",
            "C:\\Windows\\win.ini",
            "c:relative",
            "\\\\server\\share\\x",
            "//server/share/x",
            "\\\\?\\C:\\x",
            "../outside",
            "a/../../outside",
            "a\\..\\..\\outside",
            "a/..",
            "nul\0byte",
        ] {
            assert!(safe_rel_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn validates_commit_ids() {
        assert!(validate_sha("abcd").is_ok());
        assert!(validate_sha("0123456789abcdefABCDEF0123456789abcdef01").is_ok());
        assert!(validate_sha(&"a".repeat(64)).is_ok());
        for bad in ["abc", "", "HEAD", "main", "--output=x", "abcd\nexec rm -rf /", "abcd efgh", "ééééééé"] {
            assert!(validate_sha(bad).is_err(), "{bad:?}");
        }
        assert!(validate_sha(&"a".repeat(65)).is_err());
    }
}
