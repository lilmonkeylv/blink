use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_deep_link::DeepLinkExt;

pub const LAUNCH: &str = "launch";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Launch {
    Open { path: String },
    Clone { url: String },
}

static PENDING: Mutex<Vec<Launch>> = Mutex::new(Vec::new());

pub fn parse_url(raw: &str) -> Option<Launch> {
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "blink" {
        return None;
    }
    let param = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    match url.host_str()? {
        "open" => param("path").map(|path| Launch::Open { path }),
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
            path.is_dir().then(|| Launch::Open { path: strip_verbatim(&path) })
        })
        .collect()
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
        assert_eq!(parse_url("blink://open?path=C%3A%2Fx"), Some(Launch::Open { path: "C:/x".into() }));
    }

    #[test]
    fn args_resolve_directories_relative_to_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("repo")).unwrap();
        let launches =
            parse_args(&["blink".into(), "repo".into(), "--flag".into(), "missing".into()], Some(dir.path()));
        assert_eq!(launches.len(), 1);
        assert!(matches!(&launches[0], Launch::Open { path } if path.ends_with("repo")));
    }
}
