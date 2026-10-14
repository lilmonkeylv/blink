use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_deep_link::DeepLinkExt;

pub const LAUNCH: &str = "launch";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Launch {
    Open {
        path: String,
        #[serde(rename = "fromLink")]
        from_link: bool,
    },
    Clone {
        url: String,
    },
}

static PENDING: Mutex<Vec<Launch>> = Mutex::new(Vec::new());

pub fn parse_url(raw: &str) -> Option<Launch> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "blink" {
        return None;
    }
    let param = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    match url.host_str()? {
        "open" => param("path").and_then(|raw| local_dir(&raw)).map(|path| Launch::Open { path, from_link: true }),
        "clone" => {
            param("url").filter(|u| u.starts_with("https://") || u.starts_with("git@")).map(|url| Launch::Clone { url })
        }
        _ => None,
    }
}

pub fn parse_args(argv: &[String], cwd: Option<&Path>) -> Vec<Launch> {
    argv.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter_map(|arg| {
            if arg.starts_with("blink://") {
                return parse_url(arg);
            }
            let path = PathBuf::from(arg);
            let path = if path.is_absolute() { path } else { cwd.map(|c| c.join(&path)).unwrap_or(path) };
            let path = std::fs::canonicalize(&path).unwrap_or(path);
            path.is_dir().then(|| Launch::Open { path: strip_verbatim(&path), from_link: false })
        })
        .collect()
}

fn on_local_disk(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        matches!(
            path.components().next(),
            Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        )
    }
    #[cfg(not(windows))]
    {
        path.has_root()
    }
}

fn local_dir(raw: &str) -> Option<String> {
    let unc = raw.as_bytes().get(..2).is_some_and(|lead| lead.iter().all(|b| matches!(b, b'/' | b'\\')));
    let path = Path::new(raw);
    if unc || !path.is_absolute() || !on_local_disk(path) {
        return None;
    }
    let canonical = std::fs::canonicalize(path).ok()?;
    (on_local_disk(&canonical) && canonical.is_dir()).then(|| strip_verbatim(&canonical))
}

fn strip_verbatim(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

fn dispatch(app: &AppHandle, launches: Vec<Launch>) {
    if launches.is_empty() {
        return;
    }
    PENDING.lock().extend(launches);
    let _ = app.emit(LAUNCH, ());
}

pub fn handle_args(app: &AppHandle, argv: &[String], cwd: Option<&Path>) {
    dispatch(app, parse_args(argv, cwd));
}

pub fn register_deep_links(app: &AppHandle) {
    #[cfg(any(windows, target_os = "linux"))]
    if let Err(e) = app.deep_link().register_all() {
        tracing::debug!(error = %e, "deep link registration skipped");
    }
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        dispatch(app, urls.iter().filter_map(|u| parse_url(u.as_str())).collect());
    }
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        dispatch(&handle, event.urls().iter().filter_map(|u| parse_url(u.as_str())).collect());
    });
}

#[tauri::command]
pub fn take_pending_launch() -> Vec<Launch> {
    std::mem::take(&mut *PENDING.lock())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_deep_links() {
        assert_eq!(
            parse_url("blink://clone?url=https%3A%2F%2Fgithub.com%2Fa%2Fb.git"),
            Some(Launch::Clone { url: "https://github.com/a/b.git".into() })
        );
        assert_eq!(parse_url("blink://clone?url=file%3A%2F%2F%2Fetc"), None);
        assert_eq!(parse_url("https://example.com"), None);

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").unwrap();
        let open = |path: &str| {
            let encoded: String = url::form_urlencoded::byte_serialize(path.as_bytes()).collect();
            parse_url(&format!("blink://open?path={encoded}"))
        };
        let expected = strip_verbatim(&std::fs::canonicalize(dir.path()).unwrap());
        assert_eq!(open(&dir.path().to_string_lossy()), Some(Launch::Open { path: expected, from_link: true }));

        for rejected in [
            r"\\server\share\repo",
            "//server/share/repo",
            r"\/server\share",
            r"\\?\C:\Windows",
            r"\\?\UNC\server\share",
            r"\\.\C:\Windows",
            "relative/repo",
            "C:relative",
            "",
        ] {
            assert_eq!(open(rejected), None, "{rejected}");
        }
        assert_eq!(open(&file.to_string_lossy()), None);
        assert_eq!(open(&dir.path().join("missing").to_string_lossy()), None);
    }

    #[test]
    fn args_resolve_directories_relative_to_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("repo")).unwrap();
        let launches =
            parse_args(&["blink".into(), "repo".into(), "--flag".into(), "missing".into()], Some(dir.path()));
        assert_eq!(launches.len(), 1);
        assert!(matches!(&launches[0], Launch::Open { path, from_link: false } if path.ends_with("repo")));
    }

    #[test]
    fn deep_links_in_args_are_marked() {
        let dir = tempfile::tempdir().unwrap();
        let encoded: String = url::form_urlencoded::byte_serialize(dir.path().to_string_lossy().as_bytes()).collect();
        let launches = parse_args(&["blink".into(), format!("blink://open?path={encoded}")], None);
        assert!(matches!(&launches[..], [Launch::Open { from_link: true, .. }]));
    }
}
