use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use tauri::{AppHandle, State};
use tokio::sync::oneshot;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaProfile {
    pub id: String,
    pub name: String,
    pub monitor_id: String,
    pub gamma: f64,
    #[serde(default)]
    pub brightness: f64,
    #[serde(default = "default_contrast")]
    pub contrast: f64,
    pub shortcut: String,
}

fn default_contrast() -> f64 {
    1.0
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Display {
    id: String,
    label: String,
    primary: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GammaStatus {
    profiles: Vec<GammaProfile>,
    displays: Vec<Display>,
    active_profiles: std::collections::HashMap<String, String>,
    warning: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GammaAction {
    Status,
    Save {
        profiles: Vec<GammaProfile>,
    },
    Apply {
        id: String,
    },
    Reset,
    #[serde(skip)]
    Close,
}

type Request = (GammaAction, oneshot::Sender<Result<GammaStatus, String>>);

pub struct GammaTool(mpsc::Sender<Request>);

impl GammaTool {
    pub fn new(app: AppHandle) -> Self {
        let (sender, receiver) = mpsc::channel();
        #[cfg(windows)]
        let _ = std::thread::Builder::new()
            .name("gamma-tool".into())
            .spawn(move || windows::run(app, receiver));
        #[cfg(not(windows))]
        drop((app, receiver));
        Self(sender)
    }

    async fn request(&self, action: GammaAction) -> Result<GammaStatus, String> {
        let (sender, receiver) = oneshot::channel();
        self.0.send((action, sender)).map_err(|_| {
            if cfg!(windows) {
                "Gamma 工具未运行".to_string()
            } else {
                "Gamma 工具仅支持 Windows".to_string()
            }
        })?;
        receiver.await.map_err(|_| "Gamma 工具已停止".to_string())?
    }

    pub async fn close(&self) {
        let _ = self.request(GammaAction::Close).await;
    }
}

#[tauri::command]
pub async fn gamma_tool(
    tool: State<'_, GammaTool>,
    action: GammaAction,
) -> Result<GammaStatus, String> {
    tool.request(action).await
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        collections::{HashMap, HashSet},
        fs,
        io::ErrorKind,
        path::Path,
        ptr::{null, null_mut},
        time::Duration,
    };
    use tauri::{Emitter, Manager};
    use windows_sys::Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{
            CreateDCW, DeleteDC, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR,
            MONITORINFOEXW,
        },
        UI::{
            ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp},
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
                MOD_WIN,
            },
            WindowsAndMessaging::{PeekMessageW, MONITORINFOF_PRIMARY, MSG, PM_REMOVE, WM_HOTKEY},
        },
    };

    type Ramp = [[u16; 256]; 3];
    type Shortcut = (u32, u32);

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StoredProfiles {
        Current { profiles: Vec<GammaProfile> },
        Legacy(Vec<GammaProfile>),
    }

    #[derive(Default)]
    struct Worker {
        profiles: Vec<GammaProfile>,
        originals: HashMap<String, Ramp>,
        active: HashMap<String, String>,
        registrations: HashMap<Shortcut, i32>,
        next_hotkey_id: i32,
        warning: Option<String>,
    }

    pub(super) fn run(app: AppHandle, receiver: mpsc::Receiver<Request>) {
        let path = app
            .path()
            .app_config_dir()
            .map(|dir| dir.join("gamma-profiles.json"));
        let mut worker = Worker::default();
        // Create the thread message queue before registering global hotkeys.
        let mut message = MSG::default();
        unsafe { PeekMessageW(&mut message, null_mut(), WM_HOTKEY, WM_HOTKEY, PM_REMOVE) };
        match path.as_ref().map_err(ToString::to_string).and_then(|p| {
            let primary = displays()
                .into_iter()
                .next()
                .map(|display| display.id)
                .unwrap_or_else(|| r"\\.\DISPLAY1".into());
            load(p, &primary)
        }) {
            Ok(profiles) => {
                let mut warnings = Vec::new();
                for profile in &profiles {
                    match worker.prepare_bindings(std::slice::from_ref(profile)) {
                        Ok(bindings) => worker.registrations.extend(bindings),
                        Err(error) => warnings.push(error),
                    }
                }
                if !warnings.is_empty() {
                    worker.warning = Some(warnings.join("\n"));
                }
                worker.profiles = profiles;
            }
            Err(error) => worker.warning = Some(error),
        }

        loop {
            while unsafe { PeekMessageW(&mut message, null_mut(), WM_HOTKEY, WM_HOTKEY, PM_REMOVE) }
                != 0
            {
                let id = message.wParam as i32;
                if let Some(profile) = worker
                    .profiles
                    .iter()
                    .find(|profile| {
                        parse_shortcut(&profile.shortcut)
                            .ok()
                            .flatten()
                            .and_then(|shortcut| worker.registrations.get(&shortcut))
                            == Some(&id)
                    })
                    .cloned()
                {
                    let error = worker.apply(&profile.id).err();
                    let _ = app.emit(
                        "gamma-hotkey-result",
                        serde_json::json!({
                            "name": profile.name,
                            "error": error,
                            "activeProfiles": worker.active,
                        }),
                    );
                }
            }
            let wait = if worker.registrations.is_empty() {
                1000
            } else {
                30
            };
            match receiver.recv_timeout(Duration::from_millis(wait)) {
                Ok((GammaAction::Close, reply)) => {
                    let result = worker.reset().map(|()| worker.status());
                    let _ = reply.send(result);
                    break;
                }
                Ok((action, reply)) => {
                    let result = match action {
                        GammaAction::Status => Ok(()),
                        GammaAction::Save { profiles } => path
                            .as_ref()
                            .map_err(ToString::to_string)
                            .and_then(|p| worker.save(p, profiles)),
                        GammaAction::Apply { id } => worker.apply(&id),
                        GammaAction::Reset => worker.reset(),
                        GammaAction::Close => unreachable!(),
                    };
                    let _ = reply.send(result.map(|()| worker.status()));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = worker.reset();
                    break;
                }
            }
        }
        for id in worker.registrations.into_values() {
            unsafe { UnregisterHotKey(null_mut(), id) };
        }
    }

    impl Worker {
        fn status(&self) -> GammaStatus {
            GammaStatus {
                profiles: self.profiles.clone(),
                displays: displays(),
                active_profiles: self.active.clone(),
                warning: self.warning.clone(),
            }
        }

        // Register new combinations before releasing old ones so a conflict leaves the old bindings usable.
        fn prepare_bindings(
            &mut self,
            profiles: &[GammaProfile],
        ) -> Result<HashMap<Shortcut, i32>, String> {
            let shortcuts = validate(profiles)?;
            let mut additions = HashMap::new();
            for (shortcut, label) in shortcuts {
                if self.registrations.contains_key(&shortcut) {
                    continue;
                }
                self.next_hotkey_id = (self.next_hotkey_id % 0xbfff) + 1;
                while self
                    .registrations
                    .values()
                    .chain(additions.values())
                    .any(|id| *id == self.next_hotkey_id)
                {
                    self.next_hotkey_id = (self.next_hotkey_id % 0xbfff) + 1;
                }
                let (modifiers, key) = shortcut;
                let id = self.next_hotkey_id;
                if unsafe { RegisterHotKey(null_mut(), id, modifiers | MOD_NOREPEAT, key) } == 0 {
                    for id in additions.into_values() {
                        unsafe { UnregisterHotKey(null_mut(), id) };
                    }
                    return Err(format!(
                        "无法注册快捷键 {label}，可能已被 Windows 或其他程序占用"
                    ));
                }
                additions.insert(shortcut, id);
            }
            Ok(additions)
        }

        fn save(&mut self, path: &Path, profiles: Vec<GammaProfile>) -> Result<(), String> {
            let additions = self.prepare_bindings(&profiles)?;
            if let Err(error) = persist(path, &profiles) {
                for id in additions.into_values() {
                    unsafe { UnregisterHotKey(null_mut(), id) };
                }
                return Err(error);
            }
            let desired: HashSet<Shortcut> = profiles
                .iter()
                .filter_map(|profile| parse_shortcut(&profile.shortcut).ok().flatten())
                .collect();
            self.registrations.retain(|shortcut, id| {
                let keep = desired.contains(shortcut);
                if !keep {
                    unsafe { UnregisterHotKey(null_mut(), *id) };
                }
                keep
            });
            self.registrations.extend(additions);
            self.active.retain(|monitor, id| {
                profiles.iter().any(|profile| {
                    &profile.id == id
                        && &profile.monitor_id == monitor
                        && self.profiles.iter().any(|old| {
                            old.id == profile.id
                                && old.gamma == profile.gamma
                                && old.brightness == profile.brightness
                                && old.contrast == profile.contrast
                        })
                })
            });
            self.profiles = profiles;
            self.warning = None;
            Ok(())
        }

        fn apply(&mut self, id: &str) -> Result<(), String> {
            let profile = self
                .profiles
                .iter()
                .find(|profile| profile.id == id)
                .ok_or("找不到此 Gamma 配置")?;
            let device = &profile.monitor_id;
            if !displays().iter().any(|display| &display.id == device) {
                return Err("配置对应的显示器未连接".into());
            }
            let dc = DisplayContext::open(device)?;
            let before = dc.read()?;
            let original = self.originals.entry(device.clone()).or_insert(before);
            let adjusted = display_ramp(
                original,
                profile.gamma,
                profile.brightness,
                profile.contrast,
            );
            if let Err(error) = dc.write(&adjusted) {
                let _ = dc.write(&before);
                return Err(error);
            }
            self.active.insert(device.clone(), id.to_string());
            Ok(())
        }

        fn reset(&mut self) -> Result<(), String> {
            let mut errors = Vec::new();
            self.originals.retain(|device, ramp| {
                match DisplayContext::open(device).and_then(|dc| dc.write(ramp)) {
                    Ok(()) => {
                        self.active.remove(device);
                        false
                    }
                    Err(error) => {
                        errors.push(format!("{device} · {error}"));
                        true
                    }
                }
            });
            if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("\n"))
            }
        }
    }

    fn default_profiles(monitor_id: &str) -> Vec<GammaProfile> {
        [
            ("default-neutral", "默认配置", 1.0, 1.0),
            ("default-night", "默认夜间", 1.3, 1.14),
        ]
        .into_iter()
        .map(|(id, name, gamma, contrast)| GammaProfile {
            id: id.into(),
            name: name.into(),
            monitor_id: monitor_id.into(),
            gamma,
            brightness: 0.0,
            contrast,
            shortcut: String::new(),
        })
        .collect()
    }

    fn load(path: &Path, primary: &str) -> Result<Vec<GammaProfile>, String> {
        match fs::read_to_string(path) {
            Ok(content) => {
                let stored = serde_json::from_str::<StoredProfiles>(&content)
                    .map_err(|error| format!("Gamma 配置格式错误：{error}"))?;
                let profiles = match stored {
                    StoredProfiles::Current { profiles } => profiles,
                    StoredProfiles::Legacy(mut profiles) => {
                        for preset in default_profiles(primary) {
                            if !profiles.iter().any(|profile| {
                                profile.id == preset.id || profile.name == preset.name
                            }) {
                                profiles.push(preset);
                            }
                        }
                        profiles
                    }
                };
                validate(&profiles)?;
                Ok(profiles)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(default_profiles(primary)),
            Err(error) => Err(format!("无法读取 Gamma 配置：{error}")),
        }
    }

    fn persist(path: &Path, profiles: &[GammaProfile]) -> Result<(), String> {
        #[derive(Serialize)]
        struct ProfileFile<'a> {
            profiles: &'a [GammaProfile],
        }
        let json = serde_json::to_vec_pretty(&ProfileFile { profiles })
            .map_err(|error| error.to_string())?;
        let directory = path.parent().ok_or("无法获取 Gamma 配置目录")?;
        let temporary = path.with_extension("tmp");
        fs::create_dir_all(directory)
            .and_then(|()| fs::write(&temporary, json))
            .and_then(|()| fs::rename(&temporary, path))
            .map_err(|error| format!("无法保存 Gamma 配置：{error}"))
    }

    fn validate(profiles: &[GammaProfile]) -> Result<HashMap<Shortcut, &str>, String> {
        let mut ids = HashSet::new();
        let mut shortcuts = HashMap::new();
        for profile in profiles {
            if profile.id.is_empty() || !ids.insert(&profile.id) {
                return Err("Gamma 配置标识为空或重复".into());
            }
            if profile.name.trim().is_empty()
                || profile.monitor_id.is_empty()
                || !(0.7..=1.5).contains(&profile.gamma)
                || !(-0.15..=0.15).contains(&profile.brightness)
                || !(0.75..=1.25).contains(&profile.contrast)
            {
                return Err("请输入配置名称并选择显示器。Gamma 范围为 0.70–1.50，亮度为 -15%–15%，对比度为 0.75–1.25".into());
            }
            if let Some(shortcut) = parse_shortcut(&profile.shortcut)? {
                if shortcuts
                    .insert(shortcut, profile.shortcut.as_str())
                    .is_some()
                {
                    return Err(format!("快捷键 {} 已分配给另一套配置", profile.shortcut));
                }
            }
        }
        Ok(shortcuts)
    }

    fn parse_shortcut(value: &str) -> Result<Option<Shortcut>, String> {
        if value.is_empty() {
            return Ok(None);
        }
        let invalid = || format!("无效快捷键 {value}。请按修饰键加字母、数字或 F1–F11");
        let (modifiers, key) = value.rsplit_once('+').ok_or_else(invalid)?;
        let mut flags = 0;
        for part in modifiers.split('+') {
            let flag = match part {
                "Ctrl" => MOD_CONTROL,
                "Alt" => MOD_ALT,
                "Shift" => MOD_SHIFT,
                "Win" => MOD_WIN,
                _ => return Err(invalid()),
            };
            if flags & flag != 0 {
                return Err(invalid());
            }
            flags |= flag;
        }
        let vk = match key.as_bytes() {
            [byte @ (b'A'..=b'Z' | b'0'..=b'9')] => u32::from(*byte),
            _ => {
                let number = key
                    .strip_prefix('F')
                    .and_then(|n| n.parse::<u32>().ok())
                    .filter(|n| (1..=11).contains(n))
                    .ok_or_else(invalid)?;
                0x70 + number - 1
            }
        };
        Ok(Some((flags, vk)))
    }

    fn display_ramp(original: &Ramp, gamma: f64, brightness: f64, contrast: f64) -> Ramp {
        std::array::from_fn(|channel| {
            std::array::from_fn(|level| {
                let corrected = (level as f64 / 255.0).powf(1.0 / gamma);
                let adjusted = ((corrected - 0.5) * contrast + 0.5 + brightness).clamp(0.0, 1.0);
                let index = adjusted * 255.0;
                let below = index.floor() as usize;
                let fraction = index.fract();
                let lower = f64::from(original[channel][below]);
                let upper = f64::from(original[channel][index.ceil() as usize]);
                (lower + (upper - lower) * fraction).round() as u16
            })
        })
    }

    fn displays() -> Vec<Display> {
        let mut displays: Vec<Display> = Vec::new();
        unsafe extern "system" fn collect(
            monitor: HMONITOR,
            _: HDC,
            _: *mut RECT,
            context: LPARAM,
        ) -> i32 {
            let mut info = MONITORINFOEXW::default();
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
            if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) } != 0 {
                let end = info
                    .szDevice
                    .iter()
                    .position(|&ch| ch == 0)
                    .unwrap_or(info.szDevice.len());
                let id = String::from_utf16_lossy(&info.szDevice[..end]);
                let rect = info.monitorInfo.rcMonitor;
                let primary = info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;
                let label = format!(
                    "{} · {} × {}{}",
                    id.trim_start_matches(r"\\.\"),
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    if primary { "（主显示器）" } else { "" }
                );
                unsafe { &mut *(context as *mut Vec<Display>) }.push(Display {
                    id,
                    label,
                    primary,
                });
            }
            1
        }
        unsafe {
            EnumDisplayMonitors(
                null_mut(),
                null(),
                Some(collect),
                &mut displays as *mut Vec<Display> as LPARAM,
            )
        };
        displays.sort_by(|left, right| {
            right
                .primary
                .cmp(&left.primary)
                .then(left.id.cmp(&right.id))
        });
        displays
    }

    struct DisplayContext(HDC);

    impl DisplayContext {
        fn open(device: &str) -> Result<Self, String> {
            let name: Vec<u16> = device.encode_utf16().chain([0]).collect();
            let dc = unsafe { CreateDCW(null(), name.as_ptr(), null(), null()) };
            if dc.is_null() {
                Err("显示器未连接或无法打开".into())
            } else {
                Ok(Self(dc))
            }
        }

        fn read(&self) -> Result<Ramp, String> {
            let mut ramp = [[0; 256]; 3];
            if unsafe { GetDeviceGammaRamp(self.0, ramp.as_mut_ptr().cast()) } == 0 {
                return Err("无法读取显示器的 Gamma 曲线，请检查显卡驱动与 SDR 模式".into());
            }
            Ok(ramp)
        }

        fn write(&self, ramp: &Ramp) -> Result<(), String> {
            if unsafe { SetDeviceGammaRamp(self.0, ramp.as_ptr().cast()) } == 0 {
                return Err(
                    "无法设置显示器 Gamma，请在 SDR 模式下检查驱动并尝试以管理员身份运行".into(),
                );
            }
            if self
                .read()?
                .iter()
                .flatten()
                .zip(ramp.iter().flatten())
                .any(|(now, target)| now.abs_diff(*target) > 512)
            {
                return Err("显卡驱动未采用请求的 Gamma 曲线，请检查 HDR 或显示校准软件".into());
            }
            Ok(())
        }
    }

    impl Drop for DisplayContext {
        fn drop(&mut self) {
            unsafe { DeleteDC(self.0) };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn gamma_preserves_calibration_at_one_and_adjusts_midtones() {
            let original = std::array::from_fn(|channel| {
                std::array::from_fn(|level| (channel + level * 255) as u16)
            });
            assert_eq!(display_ramp(&original, 1.0, 0.0, 1.0), original);
            let darker = display_ramp(&original, 0.7, 0.0, 1.0);
            let brighter = display_ramp(&original, 1.5, 0.0, 1.0);
            for channel in 0..3 {
                assert_eq!(darker[channel][0], original[channel][0]);
                assert_eq!(brighter[channel][255], original[channel][255]);
                assert!(darker[channel][127] < original[channel][127]);
                assert!(brighter[channel][127] > original[channel][127]);
                assert!(brighter[channel].windows(2).all(|pair| pair[0] <= pair[1]));
            }
        }

        #[test]
        fn brightness_and_contrast_adjust_the_existing_curve() {
            let original =
                [[0u16; 256]; 3].map(|_| std::array::from_fn(|level| (level * 257) as u16));
            let brighter = display_ramp(&original, 1.0, 0.1, 1.0);
            let higher_contrast = display_ramp(&original, 1.0, 0.0, 1.25);
            assert!(brighter[0][32] > original[0][32]);
            assert_eq!(brighter[0][255], u16::MAX);
            assert!(higher_contrast[0][64] < original[0][64]);
            assert!(higher_contrast[0][192] > original[0][192]);
        }

        #[test]
        fn existing_profiles_default_to_neutral_brightness_and_contrast() {
            let profile: GammaProfile = serde_json::from_str(
                r#"{"id":"old","name":"Old","monitorId":"DISPLAY1","gamma":1.2,"shortcut":""}"#,
            )
            .unwrap();
            assert_eq!(profile.brightness, 0.0);
            assert_eq!(profile.contrast, 1.0);
            assert_eq!(profile.gamma, 1.2);
        }

        #[test]
        fn shortcut_order_does_not_allow_duplicate_bindings() {
            let profile = |id: &str, shortcut: &str| GammaProfile {
                id: id.into(),
                name: id.into(),
                monitor_id: "DISPLAY1".into(),
                gamma: 1.0,
                brightness: 0.0,
                contrast: 1.0,
                shortcut: shortcut.into(),
            };
            assert_eq!(parse_shortcut("Ctrl+2"), Ok(Some((MOD_CONTROL, 0x32))));
            assert_eq!(parse_shortcut("Alt+F11"), Ok(Some((MOD_ALT, 0x7a))));
            assert!(parse_shortcut("Ctrl+F12").is_err());
            assert!(validate(&[profile("a", "Ctrl+Alt+F3"), profile("b", "Alt+Ctrl+F3")]).is_err());
        }

        #[test]
        fn profiles_can_be_updated_and_reloaded() {
            let directory =
                std::env::temp_dir().join(format!("tkfmap-gamma-test-{}", rand::random::<u64>()));
            let path = directory.join("gamma-profiles.json");
            assert_eq!(load(&path, "DISPLAY1").unwrap().len(), 2);
            fs::create_dir_all(&directory).unwrap();
            fs::write(&path, r#"[{"id":"one","name":"日常","monitorId":"DISPLAY1","gamma":1.2,"shortcut":"Ctrl+Alt+G"}]"#).unwrap();
            let migrated = load(&path, "DISPLAY1").unwrap();
            assert_eq!(migrated.len(), 3);
            assert_eq!(migrated[0].name, "日常");
            assert_eq!(migrated[2].id, "default-night");
            let mut profiles = vec![GammaProfile {
                id: "one".into(),
                name: "日常".into(),
                monitor_id: "DISPLAY1".into(),
                gamma: 1.2,
                brightness: 0.0,
                contrast: 1.0,
                shortcut: "Ctrl+Alt+G".into(),
            }];
            persist(&path, &profiles).unwrap();
            profiles[0].gamma = 0.9;
            profiles[0].brightness = 0.08;
            profiles[0].contrast = 1.15;
            profiles[0].shortcut = "Ctrl+Alt+H".into();
            persist(&path, &profiles).unwrap();
            let restored = load(&path, "DISPLAY1").unwrap();
            fs::remove_file(&path).unwrap();
            fs::remove_dir(&directory).unwrap();
            assert_eq!(restored.len(), 1);
            assert_eq!(restored[0].gamma, 0.9);
            assert_eq!(restored[0].brightness, 0.08);
            assert_eq!(restored[0].contrast, 1.15);
            assert_eq!(restored[0].shortcut, "Ctrl+Alt+H");
        }

        #[test]
        fn default_presets_match_requested_values() {
            let presets = default_profiles("DISPLAY2");
            assert_eq!(presets.len(), 2);
            assert_eq!(presets[0].name, "默认配置");
            assert_eq!(
                (presets[0].gamma, presets[0].brightness, presets[0].contrast),
                (1.0, 0.0, 1.0)
            );
            assert_eq!(presets[1].name, "默认夜间");
            assert_eq!(
                (presets[1].gamma, presets[1].brightness, presets[1].contrast),
                (1.3, 0.0, 1.14)
            );
            assert!(presets
                .iter()
                .all(|preset| preset.monitor_id == "DISPLAY2" && preset.shortcut.is_empty()));
        }
    }
}
