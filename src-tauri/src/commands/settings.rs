use std::collections::HashMap;

use blink_store::Settings;
use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.read().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    settings.save(&state.paths.settings_file())?;
    let previous = std::mem::replace(&mut *state.settings.write(), settings.clone());
    crate::apply::apply_settings(&app, Some(&previous), &settings);
    crate::tray::sync(&app);
    Ok(())
}

#[tauri::command]
pub async fn check_git_executable(path: String) -> CmdResult<String> {
    let git = std::path::PathBuf::from(path.trim());
    blink_git::git_version(&git).map_err(|e| match e {
        blink_git::Error::GitNotFound => CommandError::new("gitNotFound", format!("{} does not exist", git.display())),
        other => other.into(),
    })
}

#[tauri::command]
pub fn open_logs_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let dir = state.paths.log_dir.clone();
    std::fs::create_dir_all(&dir).map_err(|e| CommandError::new("io", e.to_string()))?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[tauri::command]
pub fn take_crash_report(state: State<'_, AppState>) -> Option<String> {
    crate::crash::take_pending(&state.paths.log_dir).map(|p| p.to_string_lossy().into_owned())
}

const KEYMAP_TEMPLATE: &str = "# Blink keyboard shortcuts. Uncomment a line to change it, or set it to \"\" to turn it off.\n# Changes apply when you switch back to Blink.\n[bindings]\n# \"sync.push\" = \"Ctrl+P\"\n# \"sync.pull\" = \"Ctrl+Shift+P\"\n# \"repo.list\" = \"Ctrl+T\"\n# \"palette\" = \"Ctrl+K\"\n";

#[derive(Deserialize, Default)]
struct KeymapFile {
    #[serde(default)]
    bindings: HashMap<String, String>,
}

#[tauri::command]
pub fn keymap(state: State<'_, AppState>) -> CmdResult<HashMap<String, String>> {
    match std::fs::read_to_string(state.paths.config_dir.join("keymap.toml")) {
        Ok(text) => toml::from_str::<KeymapFile>(&text)
            .map(|k| k.bindings)
            .map_err(|e| CommandError::new("keymap", e.message().to_string())),
        Err(_) => Ok(HashMap::new()),
    }
}

#[tauri::command]
pub fn open_keymap(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let path = state.paths.config_dir.join("keymap.toml");
    if !path.exists() {
        std::fs::write(&path, KEYMAP_TEMPLATE).map_err(|e| CommandError::new("io", e.to_string()))?;
    }
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| CommandError::new("opener", e.to_string()))
}

#[tauri::command]
pub fn open_devtools(window: tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    window.open_devtools();
    #[cfg(not(debug_assertions))]
    let _ = window;
}
