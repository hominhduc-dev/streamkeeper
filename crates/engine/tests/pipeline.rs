use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    routing::get,
    Router,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
use video_domain::DownloadOptions;
use video_engine::Engine;
use video_media::ffmpeg::{self, Tools};

fn tools() -> Tools {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src-tauri/binaries");
    Tools {
        ffmpeg: root.join("ffmpeg-x86_64-pc-windows-msvc.exe"),
        ffprobe: root.join("ffprobe-x86_64-pc-windows-msvc.exe"),
    }
}
async fn fixtures(path: &Path) {
    tokio::fs::create_dir_all(path).await.unwrap();
    let args = [
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=320x180:rate=24",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000",
        "-t",
        "12",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-g",
        "48",
        "-c:a",
        "aac",
        "-f",
        "hls",
        "-hls_time",
        "2",
        "-hls_playlist_type",
        "vod",
        "-hls_segment_filename",
    ]
    .map(str::to_string)
    .into_iter()
    .chain([
        path.join("seg%d.ts").to_string_lossy().into(),
        path.join("media.m3u8").to_string_lossy().into(),
    ])
    .collect::<Vec<_>>();
    ffmpeg::execute(&tools().ffmpeg, &args, &CancellationToken::new())
        .await
        .unwrap();
    tokio::fs::write(path.join("master.m3u8"),"#EXTM3U\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",NAME=\"Tiếng Việt\",LANGUAGE=\"vie\",URI=\"subs.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=320x180,SUBTITLES=\"subs\"\nmedia.m3u8\n").await.unwrap();
    tokio::fs::write(
        path.join("subs.m3u8"),
        "#EXTM3U\n#EXT-X-TARGETDURATION:12\n#EXTINF:12,\nsub.vtt\n#EXT-X-ENDLIST\n",
    )
    .await
    .unwrap();
    tokio::fs::write(path.join("sub.vtt"),"WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nXin chào Việt Nam\n\n00:00:06.000 --> 00:00:09.000\nĐây là video kiểm thử.\n").await.unwrap();
}
async fn server(root: PathBuf, counts: Arc<AtomicUsize>) -> (String, tokio::task::JoinHandle<()>) {
    let router = Router::new().fallback(get(move |req: Request<Body>| {
        let root = root.clone();
        let counts = counts.clone();
        async move {
            let filename = req.uri().path().trim_start_matches('/');
            if filename.contains("..") {
                return Response::builder().status(400).body(Body::empty()).unwrap();
            }
            if filename == "master.m3u8" {
                let body = tokio::fs::read(root.join(filename)).await.unwrap();
                return Response::builder()
                    .header("set-cookie", "session=fixture; Path=/; HttpOnly")
                    .body(Body::from(body))
                    .unwrap();
            }
            let cookie = req
                .headers()
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !cookie.contains("session=fixture") {
                return Response::builder()
                    .status(403)
                    .body(Body::from("missing cookie"))
                    .unwrap();
            }
            if filename.ends_with(".ts") {
                let n = counts.fetch_add(1, Ordering::SeqCst);
                // Cross the progress coalescing interval while multiple checkpoint writes are pending.
                tokio::time::sleep(Duration::from_millis(350)).await;
                if n == 0 {
                    return Response::builder().status(503).body(Body::empty()).unwrap();
                }
            }
            match tokio::fs::read(root.join(filename)).await {
                Ok(body) => Response::builder()
                    .status(200)
                    .body(Body::from(body))
                    .unwrap(),
                Err(_) => {
                    eprintln!("Fixture missing: {filename}");
                    Response::builder()
                        .status(StatusCode::NOT_FOUND)
                        .body(Body::empty())
                        .unwrap()
                }
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{addr}/master.m3u8"), handle)
}
async fn await_state(engine: &Engine, id: &str, states: &[&str]) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let j = engine.get(id).await.unwrap();
            if states.contains(&j.state.as_str()) {
                return;
            }
            if j.state == "failed" {
                panic!("Pipeline failed: {:?}", j.error);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn separate_fmp4_audio_and_mkv_sidecar_offset() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fixtures(&root).await;
    for (name, input, codec) in [
        ("video", "testsrc=size=320x180:rate=24", "libx264"),
        ("audio", "sine=frequency=440:sample_rate=48000", "aac"),
    ] {
        let mut args: Vec<String> = ["-v", "error", "-y", "-f", "lavfi", "-i", input, "-t", "12"]
            .map(str::to_string)
            .into();
        if name == "video" {
            args.extend(
                ["-c:v", codec, "-pix_fmt", "yuv420p", "-g", "48", "-an"].map(str::to_string),
            );
        } else {
            args.extend(["-c:a", codec, "-vn"].map(str::to_string));
        }
        args.extend(
            [
                "-f",
                "hls",
                "-hls_time",
                "2",
                "-hls_playlist_type",
                "vod",
                "-hls_segment_type",
                "fmp4",
                "-hls_fmp4_init_filename",
            ]
            .map(str::to_string),
        );
        args.push(
            root.join(format!("{name}-init.mp4"))
                .to_string_lossy()
                .into(),
        );
        args.push("-hls_segment_filename".into());
        args.push(root.join(format!("{name}%d.m4s")).to_string_lossy().into());
        args.push(root.join(format!("{name}.m3u8")).to_string_lossy().into());
        ffmpeg::execute(&tools().ffmpeg, &args, &CancellationToken::new())
            .await
            .unwrap();
        let playlist_path = root.join(format!("{name}.m3u8"));
        let playlist = tokio::fs::read_to_string(&playlist_path).await.unwrap();
        let playlist = playlist.replace(
            &root
                .join(format!("{name}-init.mp4"))
                .to_string_lossy()
                .to_string(),
            &format!("{name}-init.mp4"),
        );
        tokio::fs::write(playlist_path, playlist).await.unwrap();
    }
    tokio::fs::write(root.join("master.m3u8"), "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"English\",LANGUAGE=\"eng\",URI=\"audio.m3u8\"\n#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"s\",NAME=\"Tiếng Việt\",LANGUAGE=\"vie\",URI=\"subs.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=320x180,AUDIO=\"a\",SUBTITLES=\"s\"\nvideo.m3u8\n").await.unwrap();
    let (source, server) = server(root, Arc::new(AtomicUsize::new(0))).await;
    let engine = Engine::open(
        temp.path().join("data"),
        temp.path().join("output"),
        tools(),
    )
    .await
    .unwrap();
    let inspected = engine.inspect(source).await.unwrap();
    let job = engine
        .create(DownloadOptions {
            inspection_id: inspected.id,
            video_id: "v0".into(),
            audio_id: Some("audio0".into()),
            subtitle_id: Some("subtitle0".into()),
            subtitle_mode: "srt".into(),
            subtitle_offset: 2.,
            output_dir: engine.settings().await.output_dir,
            filename: "fmp4".into(),
            format: "mkv".into(),
        })
        .await
        .unwrap();
    await_state(&engine, &job.id, &["completed"]).await;
    let output = PathBuf::from(job.output.unwrap());
    let sub = tokio::fs::read_to_string(output.with_extension("subtitle0.srt"))
        .await
        .unwrap();
    assert!(sub.contains("00:00:03,000 --> 00:00:05,000"));
    let probe = ffmpeg::probe(&tools(), &output, &CancellationToken::new())
        .await
        .unwrap();
    assert!(probe["streams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["codec_type"] == "audio"));
    engine.shutdown().await.unwrap();
    server.abort();
}
#[tokio::test]
async fn downloads_cookie_protected_hls_and_subtitles_with_retry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fixtures(&root).await;
    let counts = Arc::new(AtomicUsize::new(0));
    let (source, server) = server(root, counts.clone()).await;
    let engine = Engine::open(
        temp.path().join("data"),
        temp.path().join("output"),
        tools(),
    )
    .await
    .unwrap();
    let mut settings = engine.settings().await;
    settings.keep_cache = true;
    engine.set_settings(settings.clone()).await.unwrap();
    let inspected = engine.inspect(source).await.unwrap();
    let job = engine
        .create(DownloadOptions {
            inspection_id: inspected.id,
            video_id: "v0".into(),
            audio_id: None,
            subtitle_id: Some("subtitle0".into()),
            subtitle_mode: "soft".into(),
            subtitle_offset: 0.,
            output_dir: settings.output_dir,
            filename: "fixture".into(),
            format: "mp4".into(),
        })
        .await
        .unwrap();
    await_state(&engine, &job.id, &["completed"]).await;
    let job = engine.get(&job.id).await.unwrap();
    assert_eq!(job.completed_segments, job.total_segments);
    assert!(counts.load(Ordering::SeqCst) > 6);
    let probe = ffmpeg::probe(
        &tools(),
        Path::new(job.output.as_ref().unwrap()),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(probe["streams"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["codec_type"] == "subtitle" && s["tags"]["language"] == "vie"));
    assert_eq!(engine.movies().await.unwrap().len(), 1);
    assert!(engine
        .movie_poster(&job.id)
        .await
        .unwrap()
        .starts_with("data:image/jpeg;base64,/9j/"));
    engine.mark_watched(&job.id, true).await.unwrap();
    assert!(engine.rename_movie(&job.id, "../bad").await.is_err());
    let protected = PathBuf::from(job.output.as_ref().unwrap()).with_file_name("existing.mp4");
    tokio::fs::write(&protected, b"preserve").await.unwrap();
    assert!(engine.rename_movie(&job.id, "existing").await.is_err());
    engine.rename_movie(&job.id, "Tên phim mới").await.unwrap();
    assert!(!Path::new(job.output.as_ref().unwrap()).exists());
    engine.shutdown().await.unwrap();
    drop(engine);
    let reopened = Engine::open(
        temp.path().join("data"),
        temp.path().join("output"),
        tools(),
    )
    .await
    .unwrap();
    assert_eq!(reopened.list().await[0].state, "completed");
    let movie = reopened.movies().await.unwrap().remove(0);
    assert!(movie.watched);
    assert_eq!(movie.title, "Tên phim mới");
    assert!(!movie.missing);
    reopened.delete_movie(&movie.id).await.unwrap();
    assert!(!Path::new(&movie.output).exists());
    assert!(reopened.movies().await.unwrap().is_empty());
    assert_eq!(tokio::fs::read(protected).await.unwrap(), b"preserve");
    reopened.shutdown().await.unwrap();
    server.abort();
}
#[tokio::test]
async fn pause_restart_resume_uses_checkpoints_and_keeps_output_safe() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fixtures(&root).await;
    let counts = Arc::new(AtomicUsize::new(0));
    let (source, server) = server(root, counts.clone()).await;
    let data = temp.path().join("data");
    let output = temp.path().join("output");
    let engine = Engine::open(data.clone(), output.clone(), tools())
        .await
        .unwrap();
    let mut settings = engine.settings().await;
    settings.concurrency = 1;
    settings.keep_cache = true;
    engine.set_settings(settings.clone()).await.unwrap();
    let inspected = engine.inspect(source).await.unwrap();
    let job = engine
        .create(DownloadOptions {
            inspection_id: inspected.id,
            video_id: "v0".into(),
            audio_id: None,
            subtitle_id: None,
            subtitle_mode: "none".into(),
            subtitle_offset: 0.,
            output_dir: settings.output_dir,
            filename: "resume".into(),
            format: "mp4".into(),
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let j = engine.get(&job.id).await.unwrap();
            if j.completed_segments >= 1. {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert!(engine.power_status().await.0);
    let mut power_settings = engine.settings().await;
    power_settings.prevent_sleep = false;
    engine.set_settings(power_settings.clone()).await.unwrap();
    assert!(!engine.power_status().await.0);
    power_settings.prevent_sleep = true;
    engine.set_settings(power_settings).await.unwrap();
    assert!(engine.power_status().await.0);
    engine.pause(&job.id).await.unwrap();
    assert!(!engine.power_status().await.0);
    engine.shutdown().await.unwrap();
    assert!(!engine.power_status().await.0);
    assert!(!Path::new(job.output.as_ref().unwrap()).exists());
    drop(engine);
    let resumed = Engine::open(data, output, tools()).await.unwrap();
    assert_eq!(resumed.get(&job.id).await.unwrap().state, "paused");
    resumed.resume(&job.id).await.unwrap();
    await_state(&resumed, &job.id, &["completed"]).await;
    assert!(!resumed.power_status().await.0);
    assert!(Path::new(job.output.as_ref().unwrap()).exists());
    assert!(counts.load(Ordering::SeqCst) <= 9);
    resumed.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn cancellation_cleans_only_job_cache_and_never_publishes_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fixtures(&root).await;
    let (source, server) = server(root, Arc::new(AtomicUsize::new(0))).await;
    let engine = Engine::open(
        temp.path().join("data"),
        temp.path().join("output"),
        tools(),
    )
    .await
    .unwrap();
    let mut settings = engine.settings().await;
    settings.concurrency = 1;
    engine.set_settings(settings.clone()).await.unwrap();
    let inspected = engine.inspect(source).await.unwrap();
    let options = DownloadOptions {
        inspection_id: inspected.id,
        video_id: "v0".into(),
        audio_id: None,
        subtitle_id: None,
        subtitle_mode: "none".into(),
        subtitle_offset: 0.,
        output_dir: settings.output_dir.clone(),
        filename: "cancel".into(),
        format: "mp4".into(),
    };
    let mut conflict = options.clone();
    conflict.filename = "existing".into();
    let protected = PathBuf::from(&settings.output_dir).join("existing.mp4");
    tokio::fs::write(&protected, b"preserve this content")
        .await
        .unwrap();
    assert!(engine.create(conflict).await.is_err());
    let job = engine.create(options).await.unwrap();
    await_state(&engine, &job.id, &["downloading"]).await;
    engine.cancel(&job.id, false).await.unwrap();
    assert_eq!(engine.get(&job.id).await.unwrap().state, "cancelled");
    assert!(!Path::new(job.output.as_ref().unwrap()).exists());
    assert!(!PathBuf::from(&settings.cache_dir).join(&job.id).exists());
    assert_eq!(
        tokio::fs::read(protected).await.unwrap(),
        b"preserve this content"
    );
    engine.shutdown().await.unwrap();
    server.abort();
}

#[test]
fn error_redaction_removes_signed_urls() {
    let result = video_engine::redact("failed https://example.test/file?token=secret123 response");
    assert!(!result.contains("secret123"));
    assert!(!result.contains("example.test"));
}

#[test]
fn legacy_settings_default_to_preventing_sleep() {
    let settings:video_domain::Settings=serde_json::from_str(r#"{"outputDir":"C:/Videos","cacheDir":"C:/Cache","concurrency":8,"keepCache":false,"theme":"dark"}"#).unwrap();
    assert!(settings.prevent_sleep);
}

#[tokio::test]
async fn mux_recovers_duplicate_dts_and_validates_output() {
    let directory = tempfile::tempdir().unwrap();
    fixtures(directory.path()).await;
    let output = directory.path().join("corrected.mp4");
    let args: Vec<String> = ["-v", "error", "-xerror", "-nostdin", "-n", "-i"]
        .map(str::to_string)
        .into_iter()
        .chain([
            directory
                .path()
                .join("media.m3u8")
                .to_string_lossy()
                .into_owned(),
            "-c".into(),
            "copy".into(),
            "-bsf:v".into(),
            "setts=dts='if(eq(N,10),PREV_OUTDTS,DTS)'".into(),
            output.to_string_lossy().into_owned(),
        ])
        .collect();
    let cancel = CancellationToken::new();
    let failure = ffmpeg::execute(&tools().ffmpeg, &args, &cancel)
        .await
        .unwrap_err();
    assert!(failure.to_string().contains("Non-monotonic DTS"));
    tokio::fs::remove_file(&output).await.unwrap();
    ffmpeg::mux(&tools().ffmpeg, &args, &output, &cancel)
        .await
        .unwrap();
    ffmpeg::validate(&tools(), &output, 12., &cancel)
        .await
        .unwrap();
}
