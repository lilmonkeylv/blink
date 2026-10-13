use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::error::{CmdResult, CommandError};

static NEXT_LABEL: AtomicU32 = AtomicU32::new(1);
static REPO_WINDOWS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn fitted(window: (u32, u32), area_size: (u32, u32), area_origin: (i32, i32)) -> ((u32, u32), (i32, i32)) {
    let width = window.0.max(area_size.0 * 3 / 4).min(area_size.0 * 9 / 10);
    let height = window.1.max(area_size.1 * 3 / 4).min(area_size.1 * 9 / 10);
    let x = area_origin.0 + (area_size.0.saturating_sub(width) / 2) as i32;
    let y = area_origin.1 + (area_size.1.saturating_sub(height) / 2) as i32;
    ((width, height), (x, y))
}

pub fn fit_to_screen(window: &WebviewWindow) {
    let monitor = window.current_monitor().ok().flatten().or_else(|| window.primary_monitor().ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else { return };
    let area = monitor.work_area();
    let ((width, height), (x, y)) =
        fitted((size.width, size.height), (area.size.width, area.size.height), (area.position.x, area.position.y));
    if (width, height) != (size.width, size.height) {
        let _ = window.set_size(PhysicalSize::new(width, height));
    }
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

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
    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(window_url(&path).into()))
        .title("Blink")
        .inner_size(1200.0, 780.0)
        .min_inner_size(860.0, 520.0)
        .decorations(!cfg!(windows))
        .build()
        .map_err(|e| CommandError::new("window", e.to_string()))?;
    fit_to_screen(&window);
    REPO_WINDOWS.lock().push((label, path));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_fit_inside_the_work_area_and_center() {
        assert_eq!(fitted((1200, 780), (1920, 1040), (0, 0)), ((1440, 780), (240, 130)));
        assert_eq!(fitted((1200, 780), (1280, 680), (0, 0)), ((1152, 612), (64, 34)));
        assert_eq!(fitted((1200, 780), (2560, 1400), (-2560, 40)), ((1920, 1050), (-2240, 215)));
        assert_eq!(fitted((1500, 975), (1920, 1040), (0, 0)), ((1500, 936), (210, 52)));
    }

    #[test]
    fn repo_path_is_query_encoded() {
        assert_eq!(window_url(r"C:\dev\my repo"), "index.html?repo=C%3A%5Cdev%5Cmy+repo");
    }
}
