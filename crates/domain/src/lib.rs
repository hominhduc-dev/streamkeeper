use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Movie {
    pub id: String,
    pub title: String,
    pub output: String,
    pub format: String,
    pub duration: f64,
    pub added_at: f64,
    pub watched: bool,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub language: Option<String>,
    pub bandwidth: Option<f64>,
    pub resolution: Option<String>,
    pub codecs: Option<String>,
    #[serde(skip_serializing, default)]
    #[ts(skip)]
    pub url: String,
    pub audio_group: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub duration: f64,
    pub video_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    pub subtitle_tracks: Vec<Track>,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub inspection_id: String,
    pub video_id: String,
    pub audio_id: Option<String>,
    pub subtitle_id: Option<String>,
    pub subtitle_mode: String,
    pub subtitle_offset: f64,
    pub output_dir: String,
    pub filename: String,
    pub format: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub state: String,
    pub stage: String,
    pub revision: f64,
    pub created_at: f64,
    pub updated_at: f64,
    pub downloaded_bytes: f64,
    pub estimated_total_bytes: Option<f64>,
    pub completed_segments: f64,
    pub total_segments: f64,
    pub speed: f64,
    pub eta: Option<f64>,
    pub duration: f64,
    pub output: Option<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub options: DownloadOptions,
    #[serde(skip_serializing, default)]
    #[ts(skip)]
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "enabled_by_default")]
    pub prevent_sleep: bool,
    pub output_dir: String,
    pub cache_dir: String,
    pub concurrency: u32,
    pub keep_cache: bool,
    pub theme: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub sleep_prevented: bool,
    pub sleep_error: Option<String>,
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub ffmpeg_version: String,
    pub data_dir: String,
}
fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub url: String,
    pub duration: f64,
    pub sequence: u64,
    pub range: Option<(u64, u64)>,
    pub init: bool,
    pub discontinuity: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub segments: Vec<Segment>,
    pub duration: f64,
    pub target_duration: f64,
}

pub fn safe_filename(name: &str) -> bool {
    let stem = name
        .trim()
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    !name.is_empty()
        && name.len() < 180
        && name == name.trim()
        && !name.ends_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
}
#[cfg(test)]
mod tests {
    #[test]
    fn protects_windows_paths() {
        for s in ["../x", "CON.mp4", "movie.", "a:b", "", " movie"] {
            assert!(!super::safe_filename(s));
        }
        assert!(super::safe_filename("Người Nhện"));
    }
}
