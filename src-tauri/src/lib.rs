mod config;
mod parser;
mod types;
mod watcher;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::config::{detect_default_paths as detect_paths_impl, load_settings, save_settings_to_disk};
use crate::types::{AppSettings, DetectedPaths, WatcherStatus};
use crate::watcher::WatcherManager;

struct AppState(Mutex<WatcherManager>);

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state
        .0
        .lock()
        .map(|m| m.settings.clone())
        .unwrap_or_default()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<WatcherStatus, String> {
    save_settings_to_disk(&app, &settings)?;
    let mut manager = state
        .0
        .lock()
        .map_err(|_| "内部状态锁定失败".to_string())?;
    Ok(manager.restart(&app, settings))
}

#[tauri::command]
fn detect_default_paths() -> DetectedPaths {
    detect_paths_impl()
}

#[tauri::command]
fn get_watcher_status(state: State<'_, AppState>) -> WatcherStatus {
    state
        .0
        .lock()
        .map(|m| m.status.clone())
        .unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let initial_settings = load_settings(app.handle());
            let mut manager = WatcherManager::new(initial_settings.clone());
            manager.restart(app.handle(), initial_settings);
            app.manage(AppState(Mutex::new(manager)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            detect_default_paths,
            get_watcher_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
