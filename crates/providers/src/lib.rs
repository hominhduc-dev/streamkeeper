use anyhow::{ensure, Context, Result};
use reqwest::{Client, Response};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use url::Url;
use video_domain::{Inspection, Manifest, Track};
use video_media::hls;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn ticket_refresh_is_single_flight() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let router=axum::Router::new().route("/api/play-ticket",axum::routing::post(move||{let counter=counter.clone();async move{counter.fetch_add(1,Ordering::SeqCst);axum::Json(serde_json::json!({"token":"fixture-ticket","expires":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64+120000}))}}));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let root = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let session = Session::new(&format!("{root}/watch/fixture")).unwrap();
        let tasks = (0..16).map(|_| {
            let session = session.clone();
            async move { session.ticket(false).await.unwrap() }
        });
        futures_util::future::join_all(tasks).await;
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        *session.ticket.lock().await = Some(("expired".into(), 0));
        assert_eq!(session.ticket(false).await.unwrap(), "fixture-ticket");
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        server.abort();
    }
}

#[derive(Clone)]
pub struct Session {
    pub client: Client,
    pub film4k: bool,
    pub page: Url,
    ticket: Arc<Mutex<Option<(String, u64)>>>,
}
impl Session {
    pub fn new(source: &str) -> Result<Self> {
        let page = Url::parse(source)?;
        ensure!(
            matches!(page.scheme(), "https" | "http"),
            "Chỉ hỗ trợ URL HTTP/HTTPS"
        );
        ensure!(
            page.username().is_empty() && page.password().is_none(),
            "URL không được chứa credentials"
        );
        let film4k = page.host_str() == Some("film4k.net") && page.path().starts_with("/watch/");
        let client = Client::builder()
            .cookie_store(true)
            .user_agent("Mozilla/5.0 VideoDownloader/0.1")
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            client,
            film4k,
            page,
            ticket: Arc::new(Mutex::new(None)),
        })
    }
    async fn ticket(&self, force: bool) -> Result<String> {
        let mut ticket = self.ticket.lock().await;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
        if !force {
            if let Some((t, expiry)) = &*ticket {
                if *expiry > now + 60000 {
                    return Ok(t.clone());
                }
            }
        }
        let response = self
            .client
            .post(self.page.join("/api/play-ticket")?)
            .header("Accept", "application/json")
            .header("Origin", self.page.origin().ascii_serialization())
            .header("Referer", self.page.as_str())
            .send()
            .await?;
        ensure!(
            response.status().is_success(),
            "Không khởi tạo được phiên phát ({})",
            response.status()
        );
        let data: serde_json::Value = response.json().await?;
        let value = data["token"]
            .as_str()
            .context("Ticket response thiếu token")?
            .to_string();
        *ticket = Some((
            value.clone(),
            data["expires"]
                .as_u64()
                .context("Ticket response thiếu expiry")?,
        ));
        Ok(value)
    }
    pub async fn response(&self, url: &str, range: Option<(u64, u64)>) -> Result<Response> {
        let parsed = Url::parse(url)?;
        ensure!(
            matches!(parsed.scheme(), "http" | "https"),
            "Media URI không hỗ trợ"
        );
        ensure!(
            parsed.username().is_empty() && parsed.password().is_none(),
            "Media URI có credentials"
        );
        let needs_ticket = self.film4k
            && parsed.host_str() == Some("film4k.net")
            && ["/api/hls/", "/api/proxy/", "/api/live/"]
                .iter()
                .any(|p| parsed.path().starts_with(p));
        for attempt in 0..4 {
            let mut request = self.client.get(parsed.clone());
            if self.film4k {
                request = request.header("Referer", self.page.as_str());
            }
            if needs_ticket {
                request = request.header("X-F4K-PT", self.ticket(false).await?);
            }
            if let Some((offset, len)) = range {
                request = request.header("Range", format!("bytes={offset}-{}", offset + len - 1));
            }
            let response = match request.send().await {
                Ok(r) => r,
                Err(_) if attempt < 3 => {
                    tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
                    continue;
                }
                Err(_) => anyhow::bail!("Không kết nối được nguồn sau nhiều lần thử"),
            };
            let status = response.status();
            if needs_ticket && (status.as_u16() == 401 || status.as_u16() == 403) && attempt == 0 {
                self.ticket(true).await?;
                continue;
            }
            if status.is_success() {
                if range.is_some() {
                    ensure!(
                        status.as_u16() == 206 && response.headers().contains_key("content-range"),
                        "Nguồn không đáp ứng byte range"
                    );
                }
                return Ok(response);
            }
            if (status.as_u16() == 429 || status.is_server_error()) && attempt < 3 {
                let delay = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1 << attempt)
                    .min(30);
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }
            anyhow::bail!(
                "Nguồn trả HTTP {}; thử khởi tạo lại hoặc kiểm tra quyền truy cập",
                status.as_u16()
            );
        }
        anyhow::bail!("Nguồn không phản hồi sau giới hạn retry")
    }
    pub async fn text(&self, url: &str) -> Result<String> {
        let mut response = self.response(url, None).await?;
        ensure!(
            response.content_length().unwrap_or(0) <= 16 * 1024 * 1024,
            "Playlist quá lớn"
        );
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                body.len() + chunk.len() <= 16 * 1024 * 1024,
                "Playlist quá lớn"
            );
            body.extend_from_slice(&chunk);
        }
        Ok(String::from_utf8(body)?)
    }
    pub async fn manifest(&self, track: &Track) -> Result<Manifest> {
        hls::parse(
            &self.text(&track.url).await?,
            &Url::parse(&track.url)?,
            &track.id,
        )
    }
    pub async fn inspect(&self) -> Result<Inspection> {
        let (title, master) = if self.film4k {
            self.ticket(false).await?;
            let slug = self.page.path().trim_start_matches("/watch/");
            ensure!(
                !slug.is_empty() && !slug.contains('/'),
                "URL phim không hợp lệ"
            );
            let api = format!("https://film4k.net/api/watch/{slug}");
            let data: serde_json::Value = serde_json::from_str(&self.text(&api).await?)?;
            let source = data["sources"][0]["url"]
                .as_str()
                .or_else(|| data["movie"]["hlsUrl"].as_str())
                .context("Không có HLS source")?;
            let title = data["movie"]["title"]["vi"]
                .as_str()
                .or_else(|| data["movie"]["title"]["en"].as_str())
                .unwrap_or(slug)
                .to_string();
            (title, self.page.join(source)?)
        } else {
            (
                self.page
                    .path_segments()
                    .and_then(|mut s| s.next_back())
                    .unwrap_or("Video HLS")
                    .to_string(),
                self.page.clone(),
            )
        };
        let text = self.text(master.as_str()).await?;
        ensure!(
            text.trim_start_matches('\u{feff}').starts_with("#EXTM3U"),
            "URL chưa được hỗ trợ: cần playlist HLS hoặc trang Film4k"
        );
        let mut videos = Vec::new();
        let mut audios = Vec::new();
        let mut subtitles = Vec::new();
        let lines = text.lines().map(str::trim).collect::<Vec<_>>();
        for (i, line) in lines.iter().enumerate() {
            let a = hls::attributes(line);
            if line.starts_with("#EXT-X-STREAM-INF:") {
                let uri = lines
                    .get(i + 1)
                    .filter(|s| !s.starts_with('#'))
                    .context("Variant thiếu URI")?;
                videos.push(Track {
                    id: format!("v{}", videos.len()),
                    kind: "video".into(),
                    label: a
                        .get("RESOLUTION")
                        .cloned()
                        .unwrap_or_else(|| "Chất lượng gốc".into()),
                    language: None,
                    bandwidth: a.get("BANDWIDTH").and_then(|s| s.parse().ok()),
                    resolution: a.get("RESOLUTION").cloned(),
                    codecs: a.get("CODECS").cloned(),
                    url: master.join(uri)?.to_string(),
                    audio_group: a.get("AUDIO").cloned(),
                });
            } else if line.starts_with("#EXT-X-MEDIA:") && a.contains_key("URI") {
                let list = match a.get("TYPE").map(String::as_str) {
                    Some("AUDIO") => &mut audios,
                    Some("SUBTITLES") => &mut subtitles,
                    _ => continue,
                };
                let kind = if a.get("TYPE").is_some_and(|s| s == "AUDIO") {
                    "audio"
                } else {
                    "subtitle"
                };
                list.push(Track {
                    id: format!("{kind}{}", list.len()),
                    kind: kind.into(),
                    label: a.get("NAME").cloned().unwrap_or_default(),
                    language: a.get("LANGUAGE").cloned(),
                    bandwidth: None,
                    resolution: None,
                    codecs: None,
                    url: master.join(&a["URI"])?.to_string(),
                    audio_group: a.get("GROUP-ID").cloned(),
                });
            }
        }
        if videos.is_empty() {
            videos.push(Track {
                id: "v0".into(),
                kind: "video".into(),
                label: "Chất lượng gốc".into(),
                language: None,
                bandwidth: None,
                resolution: None,
                codecs: None,
                url: master.to_string(),
                audio_group: None,
            });
        }
        let first = self.manifest(&videos[0]).await?;
        Ok(Inspection {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            provider: if self.film4k { "Film4k" } else { "HLS" }.into(),
            duration: first.duration,
            video_tracks: videos,
            audio_tracks: audios,
            subtitle_tracks: subtitles,
        })
    }
}
