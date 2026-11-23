use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::parser::{parse_log_line_for_map, parse_screenshot_filename};
use crate::types::{AppSettings, WatcherErrorPayload, WatcherStatus};

pub struct WatcherManager {
    screenshot_watcher: Option<RecommendedWatcher>,
    log_watcher: Option<RecommendedWatcher>,
    pub status: WatcherStatus,
    pub settings: AppSettings,
}

impl WatcherManager {
    pub fn new(initial_settings: AppSettings) -> Self {
        Self {
            screenshot_watcher: None,
            log_watcher: None,
            status: WatcherStatus::default(),
            settings: initial_settings,
        }
    }

    pub fn restart(&mut self, app: &AppHandle, new_settings: AppSettings) -> WatcherStatus {
        // 先释放旧监听器
        self.screenshot_watcher = None;
        self.log_watcher = None;
        self.settings = new_settings.clone();
        self.status = WatcherStatus::default();

        // 1. 启动截图目录监听
        self.start_screenshot_watcher(app, &new_settings);

        // 2. 启动日志目录监听（若配置了路径且开启了自动切图）
        if new_settings.auto_switch_map_by_log && !new_settings.logs_dir.trim().is_empty() {
            self.start_log_watcher(app, &new_settings);
        }

        self.status.clone()
    }

    fn start_screenshot_watcher(&mut self, app: &AppHandle, settings: &AppSettings) {
        let dir_str = settings.screenshot_dir.trim();
        if dir_str.is_empty() {
            self.status.screenshot_watching = false;
            self.status.screenshot_error = Some("未配置截图目录".to_string());
            return;
        }

        let dir_path = PathBuf::from(dir_str);
        if !dir_path.is_dir() {
            self.status.screenshot_watching = false;
            self.status.screenshot_error = Some(format!("目录不存在: {dir_str}"));
            return;
        }

        let app_handle = app.clone();
        let auto_delete = settings.auto_delete_screenshot;
        let max_keep = settings.max_screenshots_keep;
        let last_file = Arc::new(Mutex::new(String::new()));

        let watcher_res = RecommendedWatcher::new(
            move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if !matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_)
                    ) {
                        return;
                    }
                    for path in event.paths {
                        handle_screenshot_path(
                            &app_handle,
                            &path,
                            auto_delete,
                            max_keep,
                            &last_file,
                        );
                    }
                }
                Err(e) => {
                    let _ = app_handle.emit(
                        "watcher-error",
                        WatcherErrorPayload {
                            target: "screenshot".to_string(),
                            message: format!("截图监听异常: {e}"),
                        },
                    );
                }
            },
            Config::default(),
        );

        match watcher_res {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&dir_path, RecursiveMode::NonRecursive) {
                    self.status.screenshot_watching = false;
                    self.status.screenshot_error = Some(format!("无法监听目录: {e}"));
                } else {
                    self.screenshot_watcher = Some(watcher);
                    self.status.screenshot_watching = true;
                    self.status.screenshot_error = None;
                }
            }
            Err(e) => {
                self.status.screenshot_watching = false;
                self.status.screenshot_error = Some(format!("初始化监听器失败: {e}"));
            }
        }
    }

    fn start_log_watcher(&mut self, app: &AppHandle, settings: &AppSettings) {
        let dir_path = PathBuf::from(settings.logs_dir.trim());
        if !dir_path.is_dir() {
            self.status.log_watching = false;
            self.status.log_error = Some(format!("日志目录不存在: {}", settings.logs_dir));
            return;
        }

        let app_handle = app.clone();
        let offsets: Arc<Mutex<HashMap<PathBuf, u64>>> = Arc::new(Mutex::new(HashMap::new()));

        let watcher_res = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        return;
                    }
                    for path in event.paths {
                        handle_log_path(&app_handle, &path, &offsets);
                    }
                }
            },
            Config::default(),
        );

        match watcher_res {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&dir_path, RecursiveMode::Recursive) {
                    self.status.log_watching = false;
                    self.status.log_error = Some(format!("无法监听日志目录: {e}"));
                } else {
                    self.log_watcher = Some(watcher);
                    self.status.log_watching = true;
                    self.status.log_error = None;
                }
            }
            Err(e) => {
                self.status.log_watching = false;
                self.status.log_error = Some(format!("初始化日志监听器失败: {e}"));
            }
        }
    }
}

fn handle_screenshot_path(
    app: &AppHandle,
    path: &Path,
    auto_delete: bool,
    max_keep: u32,
    last_file: &Arc<Mutex<String>>,
) {
    let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
        return;
    };

    if !filename.to_ascii_lowercase().ends_with(".png") {
        return;
    }

    let Some(payload) = parse_screenshot_filename(filename) else {
        return;
    };

    // 防止同一文件触发多次 Create/Modify 事件导致重复推送
    if let Ok(mut guard) = last_file.lock() {
        if *guard == filename {
            return;
        }
        *guard = filename.to_string();
    }

    let _ = app.emit("screenshot-coord", payload);

    if auto_delete {
        let path_owned = path.to_path_buf();
        thread::spawn(move || {
            // 等待游戏释放文件句柄后再安全清理
            thread::sleep(Duration::from_millis(250));
            if max_keep == 0 {
                let _ = fs::remove_file(path_owned);
                return;
            }
            if let Some(parent) = path_owned.parent() {
                cleanup_excess_screenshots(parent, max_keep as usize);
            }
        });
    }
}

fn cleanup_excess_screenshots(dir: &Path, max_keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let p = entry.path();
            if !p.is_file() {
                return None;
            }
            let name = p.file_name()?.to_str()?;
            parse_screenshot_filename(name)?;
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, p))
        })
        .collect();

    if files.len() <= max_keep {
        return;
    }

    // 按修改时间倒序排列，仅保留最新的 max_keep 张，删除其余旧截图
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, old_path) in files.into_iter().skip(max_keep) {
        let _ = fs::remove_file(old_path);
    }
}

fn handle_log_path(
    app: &AppHandle,
    path: &Path,
    offsets: &Arc<Mutex<HashMap<PathBuf, u64>>>,
) {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return;
    };
    if !ext.eq_ignore_ascii_case("log") {
        return;
    }

    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let file_len = metadata.len();

    let mut offset_map = match offsets.lock() {
        Ok(m) => m,
        Err(_) => return,
    };

    // 首次见到已有日志文件时，从文件末尾开始增量读取，避免回放历史记录
    let start_pos = match offset_map.get(path) {
        Some(&pos) if pos <= file_len => pos,
        Some(_) => 0,
        None => {
            let init_pos = file_len.saturating_sub(4096);
            offset_map.insert(path.to_path_buf(), init_pos);
            init_pos
        }
    };

    if file_len <= start_pos {
        return;
    }

    let Ok(mut file) = File::open(path) else {
        return;
    };
    if file.seek(SeekFrom::Start(start_pos)).is_err() {
        return;
    }

    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut bytes_read = 0u64;

    while let Ok(n) = reader.read_line(&mut line) {
        if n == 0 {
            break;
        }
        bytes_read += n as u64;
        if let Some(detected) = parse_log_line_for_map(&line) {
            let _ = app.emit("map-detected", detected);
        }
        line.clear();
    }

    offset_map.insert(path.to_path_buf(), start_pos + bytes_read);
}
