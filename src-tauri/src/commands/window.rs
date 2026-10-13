use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::error::{CmdResult, CommandError};

static NEXT_LABEL: AtomicU32 = AtomicU32::new(1);
static REPO_WINDOWS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn window_url(path: &str) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new()).append_pair("repo", path).finish();
    format!("index.html?{query}")
}

#[tauri::command]
pub async fn open_window(app: AppHandle, path: String) -> CmdResult<()> {
    let existing = {
        let mut windows = REPO_WINDOWS.lock();
        windows.retain(|(label, _)| app.get_webview_window(label).is_some());
        windows.iter().find(|(_, p)| *p == path).and_then(|(label, _)| app.get_webview_window(label))
    };
    if let Some(window) = existing {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let label = format!("repo-{}", NEXT_LABEL.fetch_add(1, Ordering::Relaxed));
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(window_url(&path).into()))
        .title("Blink")
        .inner_size(1200.0, 780.0)
        .min_inner_size(860.0, 520.0)
        .build()
        .map_err(|e| CommandError::new("window", e.to_string()))?;
    REPO_WINDOWS.lock().push((label, path));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_path_is_query_encoded() {
        assert_eq!(window_url(r"C:\dev\my repo"), "index.html?repo=C%3A%5Cdev%5Cmy+repo");
    }
}
