use axum::{
    body::{Body, Bytes},
    http::{Request, Response},
    routing::get,
    Router,
};
use futures_util::stream;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use video_domain::DownloadOptions;
use video_engine::Engine;
use video_media::ffmpeg::{self, Tools};

#[derive(Default)]
struct Metrics {
    active: AtomicUsize,
    peak: AtomicUsize,
    bytes: AtomicU64,
    requests: AtomicUsize,
    errors: AtomicUsize,
    offline: AtomicUsize,
    hits: Mutex<HashMap<String, usize>>,
}
struct Active(Arc<Metrics>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
fn tools() -> Tools {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src-tauri/binaries");
    Tools {
        ffmpeg: root.join("ffmpeg-x86_64-pc-windows-msvc.exe"),
        ffprobe: root.join("ffprobe-x86_64-pc-windows-msvc.exe"),
    }
}
fn config(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Real engine, real FFmpeg, streamed local HTTP; opt in because this writes GiB and takes minutes.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "Run through scripts/soak.py; multi-minute resource benchmark"]
async fn long_vod_concurrency_16_outage_resume() {
    let duration = config("SOAK_MEDIA_SECONDS", 10800);
    let outage = config("SOAK_OUTAGE_SECONDS", 180);
    let pad_bytes = config("SOAK_SEGMENT_BYTES", 2 * 1024 * 1024) as usize;
    let chunk_ms = config("SOAK_CHUNK_MS", 50);
    assert!(duration >= 1200 && duration.is_multiple_of(10));
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    tokio::fs::create_dir_all(&root).await.unwrap();
    println!(
        "SOAK {}",
        serde_json::json!({"phase":"fixture","pid":std::process::id(),"mediaSeconds":duration,"outageSeconds":outage,"segmentBytes":pad_bytes,"chunkMs":chunk_ms})
    );
    let mut args: Vec<String> = [
        "-v",
        "error",
        "-nostdin",
        "-n",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:size=64x64:rate=1",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=8000",
        "-t",
    ]
    .map(str::to_string)
    .into();
    args.extend([
        duration.to_string(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "ultrafast".into(),
        "-g".into(),
        "10".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "16k".into(),
        "-f".into(),
        "hls".into(),
        "-hls_time".into(),
        "10".into(),
        "-hls_playlist_type".into(),
        "vod".into(),
        "-hls_segment_filename".into(),
        root.join("seg%d.ts").to_string_lossy().into(),
        root.join("media.m3u8").to_string_lossy().into(),
    ]);
    ffmpeg::execute(&tools().ffmpeg, &args, &CancellationToken::new())
        .await
        .unwrap();
    // Valid MPEG-TS null packets increase transfer size without allocating the complete film in RAM.
    let mut packet = [0xff; 188];
    packet[..4].copy_from_slice(&[0x47, 0x1f, 0xff, 0x10]);
    let padding = Bytes::from(packet.repeat(pad_bytes / 188));
    let metrics = Arc::new(Metrics::default());
    let m = metrics.clone();
    let router = Router::new().fallback(get(move |req: Request<Body>| {
        let root = root.clone();
        let m = m.clone();
        let padding = padding.clone();
        async move {
            let name = req.uri().path().trim_start_matches('/').to_string();
            if name.contains("..") {
                return Response::builder().status(400).body(Body::empty()).unwrap();
            }
            if name.ends_with(".m3u8") {
                return Response::builder()
                    .header("set-cookie", "session=soak; Path=/; HttpOnly")
                    .body(Body::from(tokio::fs::read(root.join(name)).await.unwrap()))
                    .unwrap();
            }
            if !req
                .headers()
                .get("cookie")
                .and_then(|s| s.to_str().ok())
                .unwrap_or("")
                .contains("session=soak")
            {
                return Response::builder().status(403).body(Body::empty()).unwrap();
            }
            m.requests.fetch_add(1, Ordering::SeqCst);
            *m.hits.lock().await.entry(name.clone()).or_default() += 1;
            if m.offline.load(Ordering::SeqCst) != 0 {
                m.errors.fetch_add(1, Ordering::SeqCst);
                return Response::builder().status(503).body(Body::empty()).unwrap();
            }
            let bytes = Bytes::from(tokio::fs::read(root.join(name)).await.unwrap());
            let active = m.active.fetch_add(1, Ordering::SeqCst) + 1;
            m.peak.fetch_max(active, Ordering::SeqCst);
            let guard = Active(m.clone());
            let body = stream::unfold(
                (bytes, padding, 0usize, m, guard),
                move |(bytes, pad, pos, m, guard)| async move {
                    let total = bytes.len() + pad.len();
                    if pos >= total {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(chunk_ms)).await;
                    let chunk = if pos < bytes.len() {
                        bytes.slice(pos..(pos + 65536).min(bytes.len()))
                    } else {
                        let p = pos - bytes.len();
                        pad.slice(p..(p + 65536).min(pad.len()))
                    };
                    let next = pos + chunk.len();
                    m.bytes.fetch_add(chunk.len() as u64, Ordering::SeqCst);
                    Some((Ok::<_, std::io::Error>(chunk), (bytes, pad, next, m, guard)))
                },
            );
            Response::builder().body(Body::from_stream(body)).unwrap()
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/media.m3u8", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let engine = Engine::open(
        temp.path().join("data"),
        temp.path().join("output"),
        tools(),
    )
    .await
    .unwrap();
    let mut settings = engine.settings().await;
    settings.concurrency = 16;
    settings.keep_cache = true;
    settings.prevent_sleep = false;
    engine.set_settings(settings.clone()).await.unwrap();
    let inspection = engine.inspect(url.clone()).await.unwrap();
    let manifest = video_providers::Session::new(&url)
        .unwrap()
        .manifest(&inspection.video_tracks[0])
        .await
        .unwrap();
    let job = engine
        .create(DownloadOptions {
            inspection_id: inspection.id,
            video_id: "v0".into(),
            audio_id: None,
            subtitle_id: None,
            subtitle_mode: "none".into(),
            subtitle_offset: 0.,
            output_dir: settings.output_dir,
            filename: "soak".into(),
            format: "mp4".into(),
        })
        .await
        .unwrap();
    let start = Instant::now();
    let mut cut = None;
    let mut restored = false;
    let mut failed = false;
    let mut saved = 0.;
    let mut baseline = HashMap::new();
    let timeout = Duration::from_secs(config("SOAK_TIMEOUT_SECONDS", 1800));
    loop {
        assert!(start.elapsed() < timeout, "Soak timed out");
        let current = engine.get(&job.id).await.unwrap();
        if cut.is_none() && current.completed_segments >= 32. && current.state == "downloading" {
            metrics.offline.store(1, Ordering::SeqCst);
            cut = Some(Instant::now());
            println!(
                "SOAK {}",
                serde_json::json!({"phase":"offline","completed":current.completed_segments})
            );
        }
        if current.state == "failed" && !failed {
            assert!(cut.is_some(), "Unexpected failure: {:?}", current.error);
            failed = true;
            saved = current.completed_segments;
            // Record requests for actual cached segments, not merely successful server responses.
            let checkpoint_storage = video_storage::Storage::open(&temp.path().join("data/app.db"))
                .await
                .unwrap();
            let hits = metrics.hits.lock().await;
            for segment in &manifest.segments {
                let cached = PathBuf::from(&settings.cache_dir)
                    .join(&job.id)
                    .join("segments")
                    .join(format!("{}.bin", segment.id));
                if cached.exists()
                    && checkpoint_storage
                        .segment(&job.id, &segment.id)
                        .await
                        .unwrap()
                        .is_some()
                {
                    let name = url::Url::parse(&segment.url)
                        .unwrap()
                        .path()
                        .trim_start_matches('/')
                        .to_string();
                    baseline.insert(name.clone(), *hits.get(&name).unwrap());
                }
            }
            assert!(baseline.len() as f64 >= saved);
            assert!(
                saved > 0.
                    && current
                        .output
                        .as_ref()
                        .is_some_and(|p| !std::path::Path::new(p).exists())
            );
            println!(
                "SOAK {}",
                serde_json::json!({"phase":"failedSafely","completed":saved,"error":current.error})
            );
        }
        if !restored && cut.is_some_and(|t| t.elapsed() >= Duration::from_secs(outage)) {
            metrics.offline.store(0, Ordering::SeqCst);
            restored = true;
            if failed {
                engine.resume(&job.id).await.unwrap();
            }
            println!(
                "SOAK {}",
                serde_json::json!({"phase":"restored","manualResume":failed})
            );
        }
        if current.state == "completed" {
            assert!(
                restored && failed,
                "Expected sustained outage to exhaust retry and explicit resume"
            );
            assert_eq!(current.completed_segments, current.total_segments);
            assert!(metrics.peak.load(Ordering::SeqCst) <= 16);
            assert_eq!(metrics.peak.load(Ordering::SeqCst), 16);
            let hits = metrics.hits.lock().await;
            let reused = baseline
                .iter()
                .filter(|(name, count)| hits.get(*name) == Some(*count))
                .count();
            // Every persisted segment must be reused without another HTTP request.
            assert!(
                reused == baseline.len(),
                "Checkpointed segments were fetched again"
            );
            println!(
                "SOAK {}",
                serde_json::json!({"phase":"completed","wallSeconds":start.elapsed().as_secs_f64(),"mediaSeconds":duration,"segments":current.total_segments,"downloadedBytes":current.downloaded_bytes,"serverBytes":metrics.bytes.load(Ordering::SeqCst),"requests":metrics.requests.load(Ordering::SeqCst),"http503":metrics.errors.load(Ordering::SeqCst),"peakActive":metrics.peak.load(Ordering::SeqCst),"checkpointedAtFailure":baseline.len(),"progressSegmentsAtFailure":saved,"reusedWithoutRequest":reused,"outputBytes":tokio::fs::metadata(current.output.unwrap()).await.unwrap().len()})
            );
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    engine.shutdown().await.unwrap();
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancel_sixteen_stalled_connections_without_waiting_for_network_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let started = Arc::new(AtomicUsize::new(0));
    let count = started.clone();
    let playlist = format!(
        "#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXT-X-MEDIA-SEQUENCE:0\n{}#EXT-X-ENDLIST\n",
        (0..64)
            .map(|i| format!("#EXTINF:10,\nseg{i}.ts\n"))
            .collect::<String>()
    );
    let router = Router::new().fallback(get(move |req: Request<Body>| {
        let playlist = playlist.clone();
        let count = count.clone();
        async move {
            if req.uri().path().ends_with(".m3u8") {
                return Response::new(Body::from(playlist));
            }
            count.fetch_add(1, Ordering::SeqCst);
            // A connection accepts the request but never sends headers during the test.
            tokio::time::sleep(Duration::from_secs(180)).await;
            Response::new(Body::empty())
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/media.m3u8", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let engine = Engine::open(
        temp.path().join("data"),
        temp.path().join("output"),
        tools(),
    )
    .await
    .unwrap();
    let mut settings = engine.settings().await;
    settings.concurrency = 16;
    settings.prevent_sleep = false;
    engine.set_settings(settings.clone()).await.unwrap();
    let inspection = engine.inspect(url).await.unwrap();
    let job = engine
        .create(DownloadOptions {
            inspection_id: inspection.id,
            video_id: "v0".into(),
            audio_id: None,
            subtitle_id: None,
            subtitle_mode: "none".into(),
            subtitle_offset: 0.,
            output_dir: settings.output_dir,
            filename: "stalled".into(),
            format: "mp4".into(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while started.load(Ordering::SeqCst) < 16 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let start = Instant::now();
    tokio::time::timeout(Duration::from_secs(5), engine.cancel(&job.id, false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(engine.get(&job.id).await.unwrap().state, "cancelled");
    assert!(!std::path::Path::new(job.output.as_ref().unwrap()).exists());
    assert!(!PathBuf::from(&settings.cache_dir).join(&job.id).exists());
    println!(
        "SOAK {}",
        serde_json::json!({"phase":"cancelStalled","connections":started.load(Ordering::SeqCst),"cancelSeconds":start.elapsed().as_secs_f64()})
    );
    engine.shutdown().await.unwrap();
    server.abort();
    let _ = server.await;
}
