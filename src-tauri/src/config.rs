use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::types::{AppSettings, DetectedPaths};

const SETTINGS_FILE: &str = "settings.json";

/// 自动探测系统中常见的塔科夫截图与日志目录
pub fn detect_default_paths() -> DetectedPaths {
    let screenshot_dir = dirs::document_dir()
        .map(|d| d.join("Escape from Tarkov").join("Screenshots"))
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().to_string());

    let candidate_log_roots = [
        r"C:\Battlestate Games\Escape from Tarkov\Logs",
        r"C:\Games\Escape from Tarkov\Logs",
        r"D:\Battlestate Games\Escape from Tarkov\Logs",
        r"D:\Games\Escape from Tarkov\Logs",
        r"E:\Battlestate Games\Escape from Tarkov\Logs",
        r"E:\Games\Escape from Tarkov\Logs",
        r"C:\Program Files (x86)\Battlestate Games\Escape from Tarkov\Logs",
    ];

    let logs_dir = candidate_log_roots
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().to_string());

    DetectedPaths {
        screenshot_dir,
        logs_dir,
    }
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {e}"))?;
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录: {e}"))?;
    }
    Ok(dir.join(SETTINGS_FILE))
}

pub fn load_settings(app: &AppHandle) -> AppSettings {
    let Ok(path) = settings_file_path(app) else {
        return default_with_detected();
    };

    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut parsed) = serde_json::from_str::<AppSettings>(&content) {
            if parsed.screenshot_dir.trim().is_empty() {
                if let Some(detected) = detect_default_paths().screenshot_dir {
                    parsed.screenshot_dir = detected;
                }
            }
            return parsed;
        }
    }

    default_with_detected()
}

fn default_with_detected() -> AppSettings {
    let detected = detect_default_paths();
    AppSettings {
        screenshot_dir: detected.screenshot_dir.unwrap_or_default(),
        logs_dir: detected.logs_dir.unwrap_or_default(),
        ..AppSettings::default()
    }
}

pub fn save_settings_to_disk(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let path = settings_file_path(app)?;
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| format!("序列化配置失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入配置文件失败: {e}"))?;
    Ok(())
}
