use anyhow::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;
use video_domain::{Manifest, Segment};

pub fn attributes(line: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut quoted = false;
    let mut start = 0;
    let s = line.split_once(':').map(|(_, s)| s).unwrap_or(line);
    for (i, c) in s.char_indices().chain(std::iter::once((s.len(), ','))) {
        if c == '"' {
            quoted = !quoted;
        }
        if c == ',' && !quoted {
            if let Some((k, v)) = s[start..i].split_once('=') {
                out.insert(k.trim().into(), v.trim().trim_matches('"').into());
            }
            start = i + 1;
        }
    }
    out
}
fn range(s: &str, prior: Option<(String, u64)>, uri: &str) -> Result<(u64, u64)> {
    let mut p = s.split('@');
    let len: u64 = p.next().context("Thiếu byte range")?.parse()?;
    ensure!(len > 0, "Byte range rỗng");
    let offset = match p.next() {
        Some(v) => v.parse()?,
        None => {
            let (old, end) = prior.context("Byte range thiếu offset đầu tiên")?;
            ensure!(old == uri, "Byte range implicit đổi URI");
            end
        }
    };
    ensure!(offset.checked_add(len).is_some(), "Byte range quá lớn");
    Ok((offset, len))
}
pub fn parse(text: &str, base: &Url, identity: &str) -> Result<Manifest> {
    ensure!(
        text.trim_start_matches('\u{feff}').starts_with("#EXTM3U"),
        "Nguồn không phải HLS"
    );
    ensure!(
        text.lines().any(|s| s.trim() == "#EXT-X-ENDLIST"),
        "Chỉ hỗ trợ HLS VOD đã hoàn tất"
    );
    let mut segments = Vec::new();
    let mut duration = 0.;
    let mut seq = 0u64;
    let mut pending = None;
    let mut target = 1.;
    let mut disc = false;
    let mut epoch = 0u64;
    let mut init_count = 0u64;
    let mut pending_range = None;
    let mut prior = None;
    let mut prior_map = None;
    let mut active_map = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(v) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            seq = v.parse()?;
        } else if let Some(v) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            target = v.parse()?;
        } else if let Some(v) = line.strip_prefix("#EXTINF:") {
            pending = Some(v.split(',').next().unwrap_or("").parse::<f64>()?);
        } else if let Some(v) = line.strip_prefix("#EXT-X-BYTERANGE:") {
            pending_range = Some(v.to_string());
        } else if line.starts_with("#EXT-X-KEY:") {
            ensure!(
                attributes(line).get("METHOD").is_some_and(|s| s == "NONE"),
                "Playlist mã hóa chưa được hỗ trợ"
            );
        } else if line == "#EXT-X-DISCONTINUITY" {
            disc = true;
            epoch += 1;
        } else if line.starts_with("#EXT-X-MAP:") {
            let a = attributes(line);
            let u = base
                .join(a.get("URI").context("Init thiếu URI")?)?
                .to_string();
            let r = a
                .get("BYTERANGE")
                .map(|s| range(s, prior_map.clone(), &u))
                .transpose()?;
            if let Some((start, len)) = r {
                prior_map = Some((u.clone(), start + len));
            }
            let signature = (u.clone(), r);
            if active_map.as_ref() != Some(&signature) {
                init_count += 1;
                let id = format!(
                    "{:x}",
                    Sha256::digest(format!("{identity}:init:{init_count}:{epoch}:{r:?}"))
                );
                segments.push(Segment {
                    id,
                    url: u,
                    duration: 0.,
                    sequence: seq,
                    range: r,
                    init: true,
                    discontinuity: disc,
                });
                disc = false;
                active_map = Some(signature);
            }
        } else if !line.is_empty() && !line.starts_with('#') {
            let d = pending.take().context("Media segment thiếu EXTINF")?;
            ensure!(d.is_finite() && d > 0., "Thời lượng segment không hợp lệ");
            let u = base.join(line)?.to_string();
            let r = pending_range
                .take()
                .map(|s| range(&s, prior.clone(), &u))
                .transpose()?;
            if let Some((start, len)) = r {
                prior = Some((u.clone(), start + len));
            } else {
                prior = None;
            }
            let id = format!(
                "{:x}",
                Sha256::digest(format!(
                    "{identity}:{seq}:{epoch}:{d:.6}:{r:?}:{init_count}"
                ))
            );
            segments.push(Segment {
                id,
                url: u,
                duration: d,
                sequence: seq,
                range: r,
                init: false,
                discontinuity: disc,
            });
            disc = false;
            seq += 1;
            duration += d;
        }
    }
    if segments.is_empty() {
        bail!("Playlist không có segment");
    }
    Ok(Manifest {
        segments,
        duration,
        target_duration: target,
    })
}
pub fn local_playlist(manifest: &Manifest) -> String {
    let mut s = format!(
        "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n",
        manifest.target_duration.ceil() as u64
    );
    for segment in &manifest.segments {
        if segment.discontinuity {
            s.push_str("#EXT-X-DISCONTINUITY\n");
        }
        if segment.init {
            s.push_str(&format!("#EXT-X-MAP:URI=\"segments/{}.bin\"\n", segment.id));
        } else {
            s.push_str(&format!(
                "#EXTINF:{:.6},\nsegments/{}.bin\n",
                segment.duration, segment.id
            ));
        }
    }
    s.push_str("#EXT-X-ENDLIST\n");
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attributes_preserve_quoted_commas() {
        let a = attributes("#EXT-X-STREAM-INF:CODECS=\"avc1,mp4a\",BANDWIDTH=42");
        assert_eq!(a["CODECS"], "avc1,mp4a");
    }
    #[test]
    fn maps_ranges_and_stable_ids() {
        let b = Url::parse("https://example.com/token/v.m3u8").unwrap();
        let t="#EXTM3U\n#EXT-X-MAP:URI=\"init\"\n#EXTINF:2,\n#EXT-X-BYTERANGE:20@0\nmedia\n#EXTINF:3,\n#EXT-X-BYTERANGE:10\nmedia\n#EXT-X-ENDLIST";
        let m = parse(t, &b, "v0").unwrap();
        assert_eq!(m.duration, 5.);
        assert_eq!(m.segments[2].range, Some((20, 10)));
        let b2 = Url::parse("https://example.com/other/v.m3u8").unwrap();
        assert_eq!(
            m.segments[1].id,
            parse(t, &b2, "v0").unwrap().segments[1].id
        );
    }
    #[test]
    fn rejects_encryption_and_live() {
        let b = Url::parse("https://example.com/").unwrap();
        assert!(parse("#EXTM3U\n#EXTINF:1,\nx", &b, "v0").is_err());
        assert!(parse(
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128\n#EXTINF:1,\nx\n#EXT-X-ENDLIST",
            &b,
            "v0"
        )
        .is_err());
    }
}
