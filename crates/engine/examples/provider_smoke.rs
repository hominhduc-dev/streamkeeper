use anyhow::Result;
use video_media::transform;
use video_providers::Session;
#[tokio::main]
async fn main() -> Result<()> {
    let source = std::env::args().nth(1).expect("source URL");
    let session = Session::new(&source)?;
    let i = session.inspect().await?;
    println!(
        "Title: {}; duration: {:.2}; tracks: {}/{}/{}",
        i.title,
        i.duration,
        i.video_tracks.len(),
        i.audio_tracks.len(),
        i.subtitle_tracks.len()
    );
    let video = i
        .video_tracks
        .iter()
        .find(|t| t.resolution.as_deref() == Some("1920x800"))
        .unwrap_or(&i.video_tracks[0]);
    let m = session.manifest(video).await?;
    for segment in m.segments.iter().take(2) {
        let mut response = session.response(&segment.url, segment.range).await?;
        let status = response.status();
        let kind = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let mut head = Vec::new();
        while head.len() < 8192 {
            if let Some(c) = response.chunk().await? {
                head.extend_from_slice(&c);
            } else {
                break;
            }
        }
        let offset = transform::validate_media(&head, session.film4k)?;
        println!(
            "HTTP {status}; type {kind}; PNG prefix {offset} bytes; init {}",
            segment.init
        );
    }
    for sub in i
        .subtitle_tracks
        .iter()
        .filter(|t| matches!(t.language.as_deref(), Some("vi" | "vie")))
    {
        let m = session.manifest(sub).await?;
        let text = session.text(&m.segments[0].url).await?;
        let (cues, _) = video_media::subtitles::parse(&text)?;
        println!("Vietnamese subtitle: {}; cues {}", sub.label, cues.len());
    }
    Ok(())
}
