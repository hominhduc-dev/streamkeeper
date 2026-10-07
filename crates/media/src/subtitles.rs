use anyhow::{ensure, Context, Result};
use std::collections::HashSet;
#[derive(Clone, Debug)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}
pub fn timestamp(s: &str) -> Result<f64> {
    let parts = s
        .trim()
        .replace(',', ".")
        .split(':')
        .map(str::parse::<f64>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        (2..=3).contains(&parts.len()) && parts.iter().all(|v| v.is_finite() && *v >= 0.),
        "Timestamp phụ đề không hợp lệ"
    );
    Ok(parts.iter().fold(0., |n, v| n * 60. + v))
}
pub type TimestampMap = (f64, u64);
pub fn parse(text: &str) -> Result<(Vec<Cue>, Option<TimestampMap>)> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    ensure!(text.starts_with("WEBVTT"), "Phụ đề không phải WebVTT");
    let mut map = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("X-TIMESTAMP-MAP=") {
            let mut local = None;
            let mut pts = None;
            for part in value.split(',') {
                if let Some(v) = part.strip_prefix("LOCAL:") {
                    local = Some(timestamp(v)?);
                }
                if let Some(v) = part.strip_prefix("MPEGTS:") {
                    pts = Some(v.parse::<u64>()?);
                }
            }
            map = Some((local.context("Thiếu LOCAL")?, pts.context("Thiếu MPEGTS")?));
        }
    }
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let lines = block.lines().collect::<Vec<_>>();
        if lines.first().is_some_and(|s| s.starts_with("NOTE")) {
            continue;
        }
        if let Some(index) = lines.iter().position(|s| s.contains("-->")) {
            let (a, b) = lines[index].split_once("-->").unwrap();
            let start = timestamp(a)?;
            let end = timestamp(b.split_whitespace().next().context("Thiếu end time")?)?;
            ensure!(end >= start, "Phụ đề có khoảng thời gian đảo ngược");
            let content = lines[index + 1..].join("\n");
            if !content.is_empty() {
                cues.push(Cue {
                    start,
                    end,
                    text: content,
                });
            }
        }
    }
    Ok((cues, map))
}
fn clock(n: f64) -> String {
    let n = (n.max(0.) * 1000.).round() as u64;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        n / 3600000,
        n / 60000 % 60,
        n / 1000 % 60,
        n % 1000
    )
}
pub fn merge(parts: &[String]) -> Result<String> {
    let mut cues = Vec::new();
    let mut base = None;
    let mut seen = HashSet::new();
    for part in parts {
        let (parsed, map) = parse(part)?;
        let offset = if let Some((local, pts)) = map {
            let origin = *base.get_or_insert(pts);
            let delta = if pts >= origin {
                pts - origin
            } else {
                (1u64 << 33) + pts - origin
            };
            delta as f64 / 90000. - local
        } else {
            0.
        };
        for mut cue in parsed {
            cue.start += offset;
            cue.end += offset;
            if cue.end < 0. {
                continue;
            }
            let key = format!("{}:{}:{}", clock(cue.start), clock(cue.end), cue.text);
            if seen.insert(key) {
                cues.push(cue);
            }
            ensure!(cues.len() <= 100000, "Phụ đề vượt giới hạn cue");
        }
    }
    ensure!(!cues.is_empty(), "Phụ đề không có câu hợp lệ");
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(format!(
        "WEBVTT\n\n{}",
        cues.iter()
            .map(|c| format!("{} --> {}\n{}\n\n", clock(c.start), clock(c.end), c.text))
            .collect::<String>()
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_timestamp_maps_and_duplicate_cues() {
        let a="WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00:00.000,MPEGTS:126000\n\n00:00:01.000 --> 00:00:02.000\nChào\n";
        let b="WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00:00.000,MPEGTS:1026000\n\n00:00:01.000 --> 00:00:02.000\nBạn\n";
        let merged = merge(&[a.into(), a.into(), b.into()]).unwrap();
        let (c, _) = parse(&merged).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].start, 11.);
    }
}
