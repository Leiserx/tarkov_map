mod config;
mod gamma;
mod lan_server;
mod p2p;
mod parser;
mod server_client;
mod types;
mod watcher;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::{detect_default_paths as detect_paths_impl, load_settings, save_settings_to_disk};
use crate::gamma::GammaTool;
use crate::lan_server::{LanServerManager, LanSnapshot};
use crate::p2p::P2pManager;
use crate::server_client::{ServerClientManager, ServerConnectionStatus};
use crate::types::{AppSettings, DetectedPaths, LanServerStatus, TeamRoomState, WatcherStatus};
use crate::watcher::WatcherManager;

struct AppState(Mutex<WatcherManager>);
struct IsExiting(AtomicBool);

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state
        .0
        .lock()
        .map(|m| m.settings.clone())
        .unwrap_or_default()
}

#[tauri::command]
async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    lan: State<'_, LanServerManager>,
    settings: AppSettings,
) -> Result<WatcherStatus, String> {
    save_settings_to_disk(&app, &settings)?;
    let lan_status = lan
        .restart(&app, settings.lan_share_enabled, settings.lan_share_port)
        .await;
    let _ = app.emit("lan-status-changed", &lan_status);

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

#[tauri::command]
async fn create_room(
    app: AppHandle,
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
    player_name: String,
    map_id: String,
) -> Result<TeamRoomState, String> {
    let _ = srv.leave_room(&app).await;
    p2p.create_room(&app, player_name, map_id).await
}

#[tauri::command]
async fn join_room(
    app: AppHandle,
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
    code: String,
    player_name: String,
    map_id: String,
) -> Result<TeamRoomState, String> {
    let _ = srv.leave_room(&app).await;
    p2p.join_room(&app, code, player_name, map_id).await
}

#[tauri::command]
async fn leave_room(
    app: AppHandle,
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
) -> Result<TeamRoomState, String> {
    let _ = srv.leave_room(&app).await;
    Ok(p2p.leave_room(&app).await)
}

#[tauri::command]
async fn get_team_state(
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
) -> Result<TeamRoomState, String> {
    let srv_state = srv.get_room_state().await;
    if srv_state.in_room {
        return Ok(srv_state);
    }
    Ok(p2p.get_state().await)
}

#[tauri::command]
async fn sync_team_position(
    app: AppHandle,
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
    player_name: String,
    map_id: String,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
) -> Result<(), String> {
    if srv.get_room_state().await.in_room {
        srv.sync_position(player_name, map_id, x, y, z).await;
    } else {
        p2p.sync_position(&app, player_name, map_id, x, y, z).await;
    }
    Ok(())
}

#[tauri::command]
async fn server_connect(
    app: AppHandle,
    srv: State<'_, ServerClientManager>,
    addr: String,
) -> Result<ServerConnectionStatus, String> {
    srv.connect(&app, addr).await
}

#[tauri::command]
async fn server_disconnect(
    app: AppHandle,
    srv: State<'_, ServerClientManager>,
) -> Result<(), String> {
    srv.disconnect(&app).await;
    Ok(())
}

#[tauri::command]
async fn server_get_status(
    srv: State<'_, ServerClientManager>,
) -> Result<ServerConnectionStatus, String> {
    Ok(srv.get_status().await)
}

#[tauri::command]
async fn server_list_rooms(srv: State<'_, ServerClientManager>) -> Result<(), String> {
    srv.list_rooms().await
}

#[tauri::command]
async fn server_create_room(
    app: AppHandle,
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
    room_name: String,
    passcode: String,
    player_name: String,
    map_id: String,
) -> Result<(), String> {
    p2p.leave_room(&app).await;
    srv.create_room(room_name, passcode, player_name, map_id)
        .await
}

#[tauri::command]
async fn server_join_room(
    app: AppHandle,
    p2p: State<'_, P2pManager>,
    srv: State<'_, ServerClientManager>,
    room_id: String,
    passcode: String,
    player_name: String,
    map_id: String,
) -> Result<(), String> {
    p2p.leave_room(&app).await;
    srv.join_room(room_id, passcode, player_name, map_id).await
}

#[tauri::command]
async fn get_lan_status(lan: State<'_, LanServerManager>) -> Result<LanServerStatus, String> {
    Ok(lan.get_status().await)
}

#[tauri::command]
fn update_lan_snapshot(lan: State<'_, LanServerManager>, snapshot: LanSnapshot) {
    lan.update_snapshot(snapshot);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let initial_settings = load_settings(app.handle());
            let mut manager = WatcherManager::new(initial_settings.clone());
            manager.restart(app.handle(), initial_settings.clone());
            app.manage(AppState(Mutex::new(manager)));
            app.manage(IsExiting(AtomicBool::new(false)));
            app.manage(P2pManager::new());
            app.manage(ServerClientManager::new());
            app.manage(GammaTool::new(app.handle().clone()));

            let lan_mgr = LanServerManager::new();
            app.manage(lan_mgr);

            if initial_settings.lan_share_enabled {
                let handle = app.handle().clone();
                let port = initial_settings.lan_share_port;
                tauri::async_runtime::spawn(async move {
                    let lan = handle.state::<LanServerManager>();
                    let status = lan.restart(&handle, true, port).await;
                    let _ = handle.emit("lan-status-changed", &status);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let exiting = window.app_handle().state::<IsExiting>();
                if exiting.0.swap(true, Ordering::SeqCst) {
                    return;
                }
                api.prevent_close();
                let app = window.app_handle().clone();
                let win = window.clone();
                tauri::async_runtime::spawn(async move {
                    app.state::<GammaTool>().close().await;
                    let p2p = app.state::<P2pManager>();
                    let srv = app.state::<ServerClientManager>();
                    let lan = app.state::<LanServerManager>();
                    let _ = srv.leave_room(&app).await;
                    srv.disconnect(&app).await;
                    p2p.leave_room(&app).await;
                    lan.stop().await;
                    let _ = win.destroy();
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            detect_default_paths,
            get_watcher_status,
            create_room,
            join_room,
            leave_room,
            get_team_state,
            sync_team_position,
            server_connect,
            server_disconnect,
            server_get_status,
            server_list_rooms,
            server_create_room,
            server_join_room,
            get_lan_status,
            gamma::gamma_tool,
            update_lan_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
