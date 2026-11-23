use serde::{Deserialize, Serialize};

fn default_max_screenshots_keep() -> u32 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub player_name: String,
    pub screenshot_dir: String,
    pub logs_dir: String,
    pub auto_delete_screenshot: bool,
    #[serde(default = "default_max_screenshots_keep")]
    pub max_screenshots_keep: u32,
    pub auto_switch_map_by_log: bool,
    pub auto_center_on_coord: bool,
    pub last_selected_map_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            player_name: String::new(),
            screenshot_dir: String::new(),
            logs_dir: String::new(),
            auto_delete_screenshot: false,
            max_screenshots_keep: default_max_screenshots_keep(),
            auto_switch_map_by_log: true,
            auto_center_on_coord: true,
            last_selected_map_id: "customs".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatus {
    pub screenshot_watching: bool,
    pub screenshot_error: Option<String>,
    pub log_watching: bool,
    pub log_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DetectedPaths {
    pub screenshot_dir: Option<String>,
    pub logs_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotCoordPayload {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub qx: f64,
    pub qy: f64,
    pub qz: f64,
    pub qw: f64,
    pub filename: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MapDetectedPayload {
    pub map_id: String,
    pub raw_location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherErrorPayload {
    pub target: String,
    pub message: String,
}
