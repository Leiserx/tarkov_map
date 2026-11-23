use regex::Regex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{MapDetectedPayload, ScreenshotCoordPayload};

fn screenshot_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^\d{4}-\d{2}-\d{2}\[\d{2}-\d{2}\]_(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?),\s*(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?),\s*(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?)_(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?),\s*(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?),\s*(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?),\s*(-?\d+(?:\.\d+)?(?:e[+-]?\d+)?)_.*\.png$"
        )
        .expect("screenshot regex must compile")
    })
}

fn location_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(?:Location:\s*|"location":\s*"|scene\s+"|scene preset path:\s*maps/)([a-z0-9_]+)"#
        )
        .expect("location regex must compile")
    })
}

/// 解析塔科夫截图文件名中的坐标 (X, Y, Z) 与四元数 (qX, qY, qZ, qW)
pub fn parse_screenshot_filename(filename: &str) -> Option<ScreenshotCoordPayload> {
    let caps = screenshot_regex().captures(filename.trim())?;

    let x: f64 = caps.get(1)?.as_str().parse().ok()?;
    let y: f64 = caps.get(2)?.as_str().parse().ok()?;
    let z: f64 = caps.get(3)?.as_str().parse().ok()?;
    let qx: f64 = caps.get(4)?.as_str().parse().ok()?;
    let qy: f64 = caps.get(5)?.as_str().parse().ok()?;
    let qz: f64 = caps.get(6)?.as_str().parse().ok()?;
    let qw: f64 = caps.get(7)?.as_str().parse().ok()?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Some(ScreenshotCoordPayload {
        x,
        y,
        z,
        qx,
        qy,
        qz,
        qw,
        filename: filename.to_string(),
        timestamp,
    })
}

/// 将游戏内部地图标识映射为前端统一地图 ID
pub fn normalize_tarkov_location(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "bigmap" | "customs" => Some("customs"),
        "woods" => Some("woods"),
        "interchange" | "shopping_mall" => Some("interchange"),
        "rezervbase" | "reserve" => Some("reserve"),
        "shoreline" => Some("shoreline"),
        "lighthouse" => Some("lighthouse"),
        "tarkovstreets" | "streets" | "city" => Some("streets-of-tarkov"),
        "laboratory" | "labs" | "lab" => Some("the-lab"),
        "sandbox" | "sandbox_high" | "groundzero" => Some("ground-zero"),
        "factory4_day" | "factory4_night" | "factory" => Some("factory"),
        "terminal" => Some("terminal"),
        _ => None,
    }
}

/// 从游戏日志单行文本中识别当前加载的地图
pub fn parse_log_line_for_map(line: &str) -> Option<MapDetectedPayload> {
    let caps = location_regex().captures(line)?;
    let raw = caps.get(1)?.as_str();
    let map_id = normalize_tarkov_location(raw)?;
    Some(MapDetectedPayload {
        map_id: map_id.to_string(),
        raw_location: raw.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_screenshot_filename() {
        let name = "2024-05-01[12-30]_-123.45, 12.34, 456.78_0.000, 0.707, 0.000, 0.707_16.84.png";
        let res = parse_screenshot_filename(name).expect("should parse valid filename");
        assert!((res.x - (-123.45)).abs() < 1e-6);
        assert!((res.y - 12.34).abs() < 1e-6);
        assert!((res.z - 456.78).abs() < 1e-6);
        assert!((res.qx - 0.0).abs() < 1e-6);
        assert!((res.qy - 0.707).abs() < 1e-6);
        assert!((res.qz - 0.0).abs() < 1e-6);
        assert!((res.qw - 0.707).abs() < 1e-6);
    }

    #[test]
    fn test_parse_invalid_screenshot_filename() {
        assert!(parse_screenshot_filename("random_image.png").is_none());
        assert!(parse_screenshot_filename("2024-05-01[12-30]_1,2,3_0,0,0,1_1.jpg").is_none());
    }

    #[test]
    fn test_parse_log_line_for_map() {
        let line = r#"2024-05-01 12:30:00.123|application|Location: bigmap, Matching..."#;
        let res = parse_log_line_for_map(line).expect("should detect customs");
        assert_eq!(res.map_id, "customs");
        assert_eq!(res.raw_location, "bigmap");

        let line2 = r#"{"location":"tarkovstreets","raidMode":"Online"}"#;
        let res2 = parse_log_line_for_map(line2).expect("should detect streets");
        assert_eq!(res2.map_id, "streets-of-tarkov");

        let line3 = r#"2024-05-01 12:30:00.123|application|scene preset path:maps/shopping_mall.bundle rcid:Shopping_Mall.ScenesPreset.asset"#;
        let res3 = parse_log_line_for_map(line3).expect("should detect interchange offline/online");
        assert_eq!(res3.map_id, "interchange");
    }
}
