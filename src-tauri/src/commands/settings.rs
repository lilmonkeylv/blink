use blink_store::Settings;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.read().clone()
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    settings.save(&state.paths.settings_file())?;
    *state.settings.write() = settings;
    Ok(())
}

#[tauri::command]
pub fn open_logs_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let dir = state.paths.log_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| CommandError::new("io", e.to_string()))?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    window.open_devtools();
    #[cfg(not(debug_assertions))]
    let _ = window;
}
