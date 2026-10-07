use anyhow::{ensure, Context, Result};
use futures_util::{stream, StreamExt};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{broadcast, Mutex, Notify, RwLock},
};
use tokio_util::sync::CancellationToken;
use video_domain::*;
use video_media::{
    ffmpeg::{self, Tools},
    hls, transform,
};
use video_providers::Session;
use video_storage::Storage;

pub struct Engine {
    pub data_dir: PathBuf,
    pub tools: Tools,
    pub storage: Storage,
    jobs: RwLock<HashMap<String, Job>>,
    settings: RwLock<Settings>,
    inspections: Mutex<HashMap<String, (Inspection, Session)>>,
    controls: Mutex<HashMap<String, CancellationToken>>,
    notify: Notify,
    closing: std::sync::atomic::AtomicBool,
    pub events: broadcast::Sender<Job>,
}
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}
impl Engine {
    pub async fn open(data_dir: PathBuf, output_dir: PathBuf, tools: Tools) -> Result<Arc<Self>> {
        tokio::fs::create_dir_all(&data_dir).await?;
        let storage = Storage::open(&data_dir.join("app.db")).await?;
        let settings = storage.settings().await?.unwrap_or(Settings {
            output_dir: output_dir.to_string_lossy().into(),
            cache_dir: data_dir.join("jobs").to_string_lossy().into(),
            concurrency: 8,
            keep_cache: false,
            theme: "dark".into(),
        });
        tokio::fs::create_dir_all(&settings.cache_dir).await?;
        tokio::fs::create_dir_all(&settings.output_dir).await?;
        let mut jobs = HashMap::new();
        for mut j in storage.jobs().await? {
            if ["queued", "downloading", "muxing", "verifying"].contains(&j.state.as_str()) {
                j.state = "paused".into();
                j.stage = "interrupted".into();
                j.speed = 0.;
                j.eta = None;
                j.updated_at = now();
                storage.save(&j).await?;
            }
            jobs.insert(j.id.clone(), j);
        }
        let (events, _) = broadcast::channel(128);
        let engine = Arc::new(Self {
            data_dir,
            tools,
            storage,
            jobs: RwLock::new(jobs),
            settings: RwLock::new(settings),
            inspections: Mutex::new(HashMap::new()),
            controls: Mutex::new(HashMap::new()),
            notify: Notify::new(),
            closing: std::sync::atomic::AtomicBool::new(false),
            events,
        });
        let worker = engine.clone();
        tokio::spawn(async move { worker.worker().await });
        Ok(engine)
    }
    pub async fn inspect(&self, source: String) -> Result<Inspection> {
        let session = Session::new(&source)?;
        let inspection = session.inspect().await?;
        let mut cache = self.inspections.lock().await;
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(inspection.id.clone(), (inspection.clone(), session));
        Ok(inspection)
    }
    pub async fn list(&self) -> Vec<Job> {
        let mut jobs = self.jobs.read().await.values().cloned().collect::<Vec<_>>();
        jobs.sort_by(|a, b| b.created_at.total_cmp(&a.created_at));
        jobs
    }
    pub async fn get(&self, id: &str) -> Result<Job> {
        self.jobs
            .read()
            .await
            .get(id)
            .cloned()
            .context("Không tìm thấy tác vụ")
    }
    pub async fn settings(&self) -> Settings {
        self.settings.read().await.clone()
    }
    pub async fn set_settings(&self, s: Settings) -> Result<()> {
        ensure!(
            (1..=16).contains(&s.concurrency),
            "Concurrency phải từ 1 đến 16"
        );
        ensure!(
            ["dark", "light", "system"].contains(&s.theme.as_str()),
            "Theme không hợp lệ"
        );
        ensure!(
            !s.output_dir.trim().is_empty() && !s.cache_dir.trim().is_empty(),
            "Thiếu thư mục"
        );
        ensure!(
            !self.jobs.read().await.values().any(|j| [
                "queued",
                "downloading",
                "muxing",
                "verifying",
                "paused",
                "failed"
            ]
            .contains(&j.state.as_str()))
                || s.cache_dir == self.settings.read().await.cache_dir,
            "Không đổi cache dir khi còn job có thể tiếp tục"
        );
        tokio::fs::create_dir_all(&s.output_dir).await?;
        tokio::fs::create_dir_all(&s.cache_dir).await?;
        self.storage.set_settings(&s).await?;
        *self.settings.write().await = s;
        Ok(())
    }
    pub async fn create(&self, options: DownloadOptions) -> Result<Job> {
        ensure!(
            safe_filename(&options.filename),
            "Tên file không hợp lệ trên Windows"
        );
        ensure!(
            ["mp4", "mkv"].contains(&options.format.as_str()),
            "Container không hỗ trợ"
        );
        ensure!(
            ["none", "soft", "srt", "vtt"].contains(&options.subtitle_mode.as_str()),
            "Chế độ phụ đề không hợp lệ"
        );
        ensure!(
            options.subtitle_offset.is_finite() && options.subtitle_offset.abs() <= 86400.,
            "Offset không hợp lệ"
        );
        let (inspection, session) = self
            .inspections
            .lock()
            .await
            .get(&options.inspection_id)
            .cloned()
            .context("Phân tích đã hết hạn; hãy phân tích lại URL")?;
        let video = inspection
            .video_tracks
            .iter()
            .find(|t| t.id == options.video_id)
            .context("Video track không tồn tại")?;
        if let Some(audio) = &options.audio_id {
            let track = inspection
                .audio_tracks
                .iter()
                .find(|t| &t.id == audio)
                .context("Audio track không tồn tại")?;
            ensure!(
                video.audio_group.is_some() && video.audio_group == track.audio_group,
                "Audio không thuộc nhóm video đã chọn"
            );
        } else {
            ensure!(video.audio_group.is_none(), "Hãy chọn audio track");
        }
        if let Some(sub) = &options.subtitle_id {
            ensure!(
                inspection.subtitle_tracks.iter().any(|t| &t.id == sub),
                "Subtitle track không tồn tại"
            );
        } else {
            ensure!(options.subtitle_mode == "none", "Hãy chọn phụ đề");
        }
        let output_dir = PathBuf::from(&options.output_dir);
        ensure!(
            output_dir.is_absolute(),
            "Output dir phải là đường dẫn tuyệt đối"
        );
        tokio::fs::create_dir_all(&output_dir).await?;
        let output = output_dir.join(format!("{}.{}", options.filename, options.format));
        ensure!(!output.exists(), "File đã tồn tại; hãy đổi tên");
        ensure!(
            !self.jobs.read().await.values().any(|j| j.output.as_deref()
                == Some(output.to_string_lossy().as_ref())
                && !["failed", "cancelled"].contains(&j.state.as_str())),
            "Tên output đang được dùng bởi tác vụ khác"
        );
        let write_test = output_dir.join(format!(".write-test-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&write_test, [])
            .await
            .context("Không có quyền ghi thư mục")?;
        tokio::fs::remove_file(write_test).await?;
        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            title: inspection.title,
            provider: inspection.provider,
            state: "queued".into(),
            stage: "queued".into(),
            revision: 1.,
            created_at: now(),
            updated_at: now(),
            downloaded_bytes: 0.,
            estimated_total_bytes: None,
            completed_segments: 0.,
            total_segments: 0.,
            speed: 0.,
            eta: None,
            duration: inspection.duration,
            output: Some(output.to_string_lossy().into()),
            error: None,
            error_code: None,
            source: session.page.to_string(),
            options,
        };
        self.storage.save(&job).await?;
        self.jobs.write().await.insert(job.id.clone(), job.clone());
        let _ = self.events.send(job.clone());
        self.notify.notify_one();
        Ok(job)
    }
    async fn update(&self, id: &str, f: impl FnOnce(&mut Job)) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        let job = jobs.get_mut(id).context("Không tìm thấy job")?;
        f(job);
        job.revision += 1.;
        job.updated_at = now();
        self.storage.save(job).await?;
        let _ = self.events.send(job.clone());
        Ok(())
    }
    pub async fn pause(&self, id: &str) -> Result<()> {
        let j = self.get(id).await?;
        ensure!(
            ["queued", "downloading"].contains(&j.state.as_str()),
            "Chỉ pause khi chờ hoặc đang tải"
        );
        self.update(id, |j| {
            if ["queued", "downloading"].contains(&j.state.as_str()) {
                j.state = "paused".into();
                j.speed = 0.;
                j.eta = None;
            }
        })
        .await?;
        ensure!(
            self.get(id).await?.state == "paused",
            "Tác vụ đã chuyển sang ghép video"
        );
        if let Some(c) = self.controls.lock().await.get(id) {
            c.cancel();
        }
        Ok(())
    }
    pub async fn resume(&self, id: &str) -> Result<()> {
        let j = self.get(id).await?;
        ensure!(
            ["paused", "failed"].contains(&j.state.as_str()),
            "Tác vụ không thể tiếp tục"
        );
        ensure!(
            !self.controls.lock().await.contains_key(id),
            "Đang dừng tác vụ; thử lại sau"
        );
        ensure!(
            !Path::new(j.output.as_deref().unwrap_or("")).exists(),
            "Output đã tồn tại; hãy tạo tác vụ với tên khác"
        );
        self.update(id, |j| {
            j.state = "queued".into();
            j.stage = "queued".into();
            j.error = None;
            j.error_code = None;
        })
        .await?;
        self.notify.notify_one();
        Ok(())
    }
    pub async fn cancel(&self, id: &str, keep_cache: bool) -> Result<()> {
        let j = self.get(id).await?;
        ensure!(
            !["completed", "cancelled"].contains(&j.state.as_str()),
            "Tác vụ đã kết thúc"
        );
        self.update(id, |j| {
            if !["completed", "cancelled"].contains(&j.state.as_str()) {
                j.state = "cancelled".into();
                j.stage = "cancelled".into();
                j.speed = 0.;
                j.eta = None;
            }
        })
        .await?;
        ensure!(
            self.get(id).await?.state == "cancelled",
            "Tác vụ đã kết thúc"
        );
        if let Some(c) = self.controls.lock().await.get(id) {
            c.cancel();
        }
        if !keep_cache {
            while self.controls.lock().await.contains_key(id) {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            self.cleanup(id).await?;
        }
        Ok(())
    }
    pub async fn cleanup(&self, id: &str) -> Result<()> {
        let j = self.get(id).await?;
        ensure!(
            ["completed", "cancelled", "failed", "paused"].contains(&j.state.as_str())
                && !self.controls.lock().await.contains_key(id),
            "Tác vụ đang chạy"
        );
        ensure!(uuid::Uuid::parse_str(id).is_ok(), "Job id không hợp lệ");
        let base = PathBuf::from(self.settings.read().await.cache_dir.clone());
        let path = base.join(id);
        if path.exists() {
            let canonical = tokio::fs::canonicalize(&path).await?;
            let root = tokio::fs::canonicalize(&base).await?;
            ensure!(
                canonical.parent() == Some(root.as_path()),
                "Cache path không an toàn"
            );
            tokio::fs::remove_dir_all(path).await?;
        }
        Ok(())
    }
    async fn worker(self: Arc<Self>) {
        loop {
            if self.closing.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let wake = self.notify.notified();
            let next = self
                .jobs
                .read()
                .await
                .values()
                .filter(|j| j.state == "queued")
                .min_by(|a, b| a.created_at.total_cmp(&b.created_at))
                .map(|j| j.id.clone());
            if let Some(id) = next {
                let cancel = CancellationToken::new();
                self.controls
                    .lock()
                    .await
                    .insert(id.clone(), cancel.clone());
                let result = self.run(&id, &cancel).await;
                if result.is_err() {
                    if let Ok(job) = self.get(&id).await {
                        if let Some(output) = job.output {
                            let path = PathBuf::from(output);
                            if let Some(parent) = path.parent() {
                                let _ = tokio::fs::remove_file(
                                    parent.join(format!(".{}.partial.{}", id, job.options.format)),
                                )
                                .await;
                            }
                        }
                    }
                }
                self.controls.lock().await.remove(&id);
                if let Err(error) = result {
                    if let Ok(j) = self.get(&id).await {
                        if !["paused", "cancelled"].contains(&j.state.as_str()) {
                            let message = redact(&error.to_string());
                            let _ = self
                                .update(&id, |j| {
                                    j.state = "failed".into();
                                    j.error = Some(message);
                                    j.error_code = Some("DOWNLOAD_FAILED".into());
                                    j.speed = 0.;
                                    j.eta = None;
                                })
                                .await;
                        }
                    }
                }
                if let Ok(j) = self.get(&id).await {
                    if j.state == "completed" && !self.settings.read().await.keep_cache {
                        let _ = self.cleanup(&id).await;
                    }
                }
            } else {
                wake.await;
            }
        }
    }
    async fn run(&self, id: &str, cancel: &CancellationToken) -> Result<()> {
        let job = self.get(id).await?;
        if job.state != "queued" {
            return Ok(());
        }
        self.update(id, |j| {
            if j.state == "queued" {
                j.state = "downloading".into();
                j.stage = "inspecting".into();
                j.speed = 0.;
                j.eta = None;
            }
        })
        .await?;
        if self.get(id).await?.state != "downloading" {
            return Ok(());
        }
        let session = Session::new(&job.source)?;
        let inspection = tokio::select! {r=session.inspect()=>r?,_=cancel.cancelled()=>anyhow::bail!("CANCELLED")};
        let video = inspection
            .video_tracks
            .iter()
            .find(|t| t.id == job.options.video_id)
            .context("Track video đã thay đổi")?;
        let mut tracks = vec![video.clone()];
        if let Some(audio) = &job.options.audio_id {
            let t = inspection
                .audio_tracks
                .iter()
                .find(|t| &t.id == audio)
                .context("Track audio đã thay đổi")?;
            ensure!(video.audio_group == t.audio_group, "Nhóm audio đã thay đổi");
            tracks.push(t.clone());
        }
        let settings = self.settings.read().await.clone();
        let root = PathBuf::from(&settings.cache_dir).join(id);
        let segment_dir = root.join("segments");
        tokio::fs::create_dir_all(&segment_dir).await?;
        let mut manifests = Vec::new();
        for track in &tracks {
            let manifest = tokio::select! {r=session.manifest(track)=>r?,_=cancel.cancelled()=>anyhow::bail!("CANCELLED")};
            manifests.push(manifest);
        }
        let expected = manifests[0].duration;
        let fingerprint = serde_json::json!({"tracks":tracks.iter().map(|t|(&t.id,&t.label,&t.language,&t.codecs)).collect::<Vec<_>>(),"segments":manifests.iter().flat_map(|m|m.segments.iter().map(|s|(&s.id,s.duration))).collect::<Vec<_>>()});
        let fingerprint_path = root.join("fingerprint.json");
        if fingerprint_path.exists() {
            ensure!(
                serde_json::from_slice::<serde_json::Value>(
                    &tokio::fs::read(&fingerprint_path).await?
                )? == fingerprint,
                "Nguồn/track đã thay đổi; tạo tác vụ mới để tránh ghép sai segment"
            );
        }
        tokio::fs::write(fingerprint_path, serde_json::to_vec(&fingerprint)?).await?;
        let all = manifests
            .iter()
            .flat_map(|m| m.segments.iter().cloned())
            .collect::<Vec<_>>();
        let estimated = video.bandwidth.map(|b| b * expected / 8.);
        let output = PathBuf::from(job.output.as_ref().context("Thiếu output")?);
        ensure!(
            fs2::available_space(output.parent().context("Output dir không hợp lệ")?)?
                > estimated.unwrap_or(128. * 1024. * 1024.) as u64,
            "Không đủ dung lượng ổ đĩa output"
        );
        ensure!(
            fs2::available_space(&root)? > estimated.unwrap_or(128. * 1024. * 1024.) as u64,
            "Không đủ dung lượng cache"
        );
        if tokio::fs::canonicalize(&root).await?.components().next()
            == tokio::fs::canonicalize(output.parent().unwrap())
                .await?
                .components()
                .next()
        {
            ensure!(
                fs2::available_space(&root)? > estimated.unwrap_or(128. * 1024. * 1024.) as u64 * 2,
                "Ổ đĩa cần chỗ cho cache và output"
            );
        }
        self.update(id, |j| {
            j.stage = "downloading".into();
            j.duration = expected;
            j.total_segments = all.len() as f64;
            j.completed_segments = 0.;
            j.downloaded_bytes = 0.;
            j.estimated_total_bytes = estimated;
        })
        .await?;
        let began = Instant::now();
        let mut done = 0usize;
        let mut bytes = 0u64;
        let mut transferred = 0u64;
        let mut last = Instant::now();
        let mut tasks = stream::iter(all.into_iter().map(|segment| {
            let session = session.clone();
            let directory = segment_dir.clone();
            async move {
                self.download_segment(id, &session, &segment, &directory, cancel)
                    .await
            }
        }))
        .buffer_unordered(settings.concurrency as usize);
        while let Some(result) = tasks.next().await {
            let (size, new) = result?;
            done += 1;
            bytes += size;
            if new {
                transferred += size;
            }
            if last.elapsed().as_millis() >= 250 {
                let speed = transferred as f64 / began.elapsed().as_secs_f64().max(0.1);
                self.update(id, |j| {
                    if j.state == "downloading" {
                        j.completed_segments = done as f64;
                        j.downloaded_bytes = bytes as f64;
                        j.speed = speed;
                        j.eta = j
                            .estimated_total_bytes
                            .filter(|_| speed > 0.)
                            .map(|total| (total - bytes as f64).max(0.) / speed);
                    }
                })
                .await?;
                last = Instant::now();
            }
        }
        drop(tasks);
        ensure!(!cancel.is_cancelled(), "CANCELLED");
        self.update(id, |j| {
            j.completed_segments = done as f64;
            j.downloaded_bytes = bytes as f64;
            j.speed = 0.;
            j.eta = None;
            j.estimated_total_bytes = Some(bytes as f64);
        })
        .await?;
        let mut inputs = Vec::new();
        for (i, manifest) in manifests.iter().enumerate() {
            let path = root.join(format!("track-{i}.m3u8"));
            tokio::fs::write(&path, hls::local_playlist(manifest)).await?;
            inputs.push(path);
        }
        let subtitle = if let Some(subid) = &job.options.subtitle_id {
            if job.options.subtitle_mode != "none" {
                let track = inspection
                    .subtitle_tracks
                    .iter()
                    .find(|t| &t.id == subid)
                    .context("Track subtitle đã thay đổi")?;
                Some((
                    self.subtitle(&session, track, &root, cancel).await?,
                    track.language.clone().unwrap_or("und".into()),
                ))
            } else {
                None
            }
        } else {
            None
        };
        ensure!(!cancel.is_cancelled(), "CANCELLED");
        self.update(id, |j| {
            if j.state == "downloading" {
                j.state = "muxing".into();
                j.stage = "muxing".into();
            }
        })
        .await?;
        ensure!(self.get(id).await?.state == "muxing", "CANCELLED");
        let temp = output
            .parent()
            .unwrap()
            .join(format!(".{}.partial.{}", id, job.options.format));
        let _ = tokio::fs::remove_file(&temp).await;
        let mut args = vec![
            "-hide_banner".into(),
            "-v".into(),
            "error".into(),
            "-xerror".into(),
            "-nostdin".into(),
            "-n".into(),
        ];
        for input in &inputs {
            args.extend([
                "-protocol_whitelist".into(),
                "file,crypto,data".into(),
                "-allowed_extensions".into(),
                "ALL".into(),
                "-allowed_segment_extensions".into(),
                "ALL".into(),
                "-extension_picky".into(),
                "0".into(),
                "-i".into(),
                input.to_string_lossy().into(),
            ]);
        }
        if let Some((path, _)) = &subtitle {
            if job.options.subtitle_mode == "soft" {
                args.extend([
                    "-itsoffset".into(),
                    job.options.subtitle_offset.to_string(),
                    "-i".into(),
                    path.to_string_lossy().into(),
                ]);
            }
        }
        args.extend([
            "-map".into(),
            "0:v:0".into(),
            "-map".into(),
            if tracks.len() > 1 { "1:a:0" } else { "0:a:0" }.into(),
            "-c".into(),
            "copy".into(),
        ]);
        if let Some((_, language)) = &subtitle {
            if job.options.subtitle_mode == "soft" {
                args.extend([
                    "-map".into(),
                    format!("{}:0", inputs.len()),
                    "-c:s".into(),
                    if job.options.format == "mp4" {
                        "mov_text"
                    } else {
                        "srt"
                    }
                    .into(),
                    "-metadata:s:s:0".into(),
                    format!("language={language}"),
                    "-metadata:s:s:0".into(),
                    "title=Phụ đề".into(),
                    "-disposition:s:0".into(),
                    "default".into(),
                ]);
            }
        }
        if job.options.format == "mp4" {
            args.extend(["-movflags".into(), "+faststart".into()]);
        }
        args.push(temp.to_string_lossy().into());
        ffmpeg::execute(&self.tools.ffmpeg, &args, cancel).await?;
        ensure!(!cancel.is_cancelled(), "CANCELLED");
        self.update(id, |j| {
            if j.state == "muxing" {
                j.state = "verifying".into();
                j.stage = "verifying".into();
            }
        })
        .await?;
        ensure!(self.get(id).await?.state == "verifying", "CANCELLED");
        ffmpeg::validate(&self.tools, &temp, expected, cancel).await?;
        if let Some((path, _)) = &subtitle {
            if ["srt", "vtt"].contains(&job.options.subtitle_mode.as_str()) {
                let suboutput = output.with_extension(format!(
                    "{}.{}",
                    job.options.subtitle_id.as_deref().unwrap_or("sub"),
                    job.options.subtitle_mode
                ));
                ensure!(!suboutput.exists(), "File phụ đề đã tồn tại");
                let subtemp = root.join(format!("export.{}", job.options.subtitle_mode));
                ffmpeg::execute(
                    &self.tools.ffmpeg,
                    &[
                        "-v".into(),
                        "error".into(),
                        "-y".into(),
                        "-itsoffset".into(),
                        job.options.subtitle_offset.to_string(),
                        "-i".into(),
                        path.to_string_lossy().into(),
                        subtemp.to_string_lossy().into(),
                    ],
                    cancel,
                )
                .await?;
                tokio::fs::copy(subtemp, suboutput).await?;
            }
        }
        let mut jobs = self.jobs.write().await;
        let final_job = jobs.get_mut(id).context("Không tìm thấy job")?;
        ensure!(
            !cancel.is_cancelled() && final_job.state == "verifying",
            "CANCELLED"
        );
        // Hold the job lock through publication so cancellation cannot publish a cancelled job.
        // hard_link atomically refuses overwriting an existing final name; both paths are on the same volume.
        tokio::fs::hard_link(&temp, &output)
            .await
            .context("Không công bố được output (có thể trùng tên)")?;
        tokio::fs::remove_file(&temp).await?;
        final_job.state = "completed".into();
        final_job.stage = "completed".into();
        final_job.error = None;
        final_job.error_code = None;
        final_job.revision += 1.;
        final_job.updated_at = now();
        self.storage.save(final_job).await?;
        let _ = self.events.send(final_job.clone());
        Ok(())
    }
    async fn download_segment(
        &self,
        job: &str,
        session: &Session,
        s: &Segment,
        dir: &Path,
        cancel: &CancellationToken,
    ) -> Result<(u64, bool)> {
        let final_path = dir.join(format!("{}.bin", s.id));
        if let Some((bytes, checksum)) = self.storage.segment(job, &s.id).await? {
            if final_path.exists() {
                let (actual, hash) = hash_file(&final_path).await?;
                if actual == bytes && hash == checksum {
                    return Ok((bytes, false));
                }
            }
        }
        for attempt in 0..3 {
            let part = dir.join(format!("{}.part", s.id));
            let result:Result<(u64,String)>=async {
                let response=tokio::select!{r=session.response(&s.url,s.range)=>r?,_=cancel.cancelled()=>anyhow::bail!("CANCELLED")};
                let mut body=response.bytes_stream();let mut file=tokio::fs::File::create(&part).await?;let mut received=0u64;
                while let Some(chunk)=tokio::select!{r=body.next()=>r,_=cancel.cancelled()=>anyhow::bail!("CANCELLED")} {let chunk=chunk?;file.write_all(&chunk).await?;received+=chunk.len() as u64;}
                file.flush().await?;file.sync_all().await?;drop(file);
                if let Some((_,length))=s.range {ensure!(received==length,"Byte range response thiếu dữ liệu");}
                let mut source=tokio::fs::File::open(&part).await?;let mut head=vec![0;65536];let n=source.read(&mut head).await?;head.truncate(n);
                let offset=transform::validate_media(&head,session.film4k)?;
                if offset>0 {source.seek(std::io::SeekFrom::Start(offset as u64)).await?;let clean=dir.join(format!("{}.clean",s.id));let mut dest=tokio::fs::File::create(&clean).await?;tokio::io::copy(&mut source,&mut dest).await?;dest.sync_all().await?;drop(dest);drop(source);tokio::fs::remove_file(&part).await?;tokio::fs::rename(clean,&part).await?;}else{drop(source);}
                let (size,checksum)=hash_file(&part).await?;if final_path.exists(){tokio::fs::remove_file(&final_path).await?;}tokio::fs::rename(&part,&final_path).await?;Ok((size,checksum))
            }.await;
            match result {
                Ok((size, checksum)) => {
                    self.storage.checkpoint(job, &s.id, size, &checksum).await?;
                    return Ok((size, true));
                }
                Err(error) => {
                    let _ = tokio::fs::remove_file(&part).await;
                    if cancel.is_cancelled() || attempt == 2 {
                        return Err(error);
                    }
                    tokio::select! {_=tokio::time::sleep(std::time::Duration::from_secs(attempt+1))=>{},_=cancel.cancelled()=>anyhow::bail!("CANCELLED")};
                }
            }
        }
        anyhow::bail!("Tải segment thất bại")
    }
    async fn subtitle(
        &self,
        session: &Session,
        track: &Track,
        root: &Path,
        cancel: &CancellationToken,
    ) -> Result<PathBuf> {
        let text = tokio::select! {r=session.text(&track.url)=>r?,_=cancel.cancelled()=>anyhow::bail!("CANCELLED")};
        let path = root.join("subtitle.vtt");
        let parts = if text.trim_start().starts_with("#EXTM3U") {
            let manifest = hls::parse(&text, &url::Url::parse(&track.url)?, &track.id)?;
            let mut parts = Vec::new();
            for segment in manifest.segments {
                ensure!(
                    !segment.init && segment.range.is_none(),
                    "Subtitle init/byte range chưa được hỗ trợ"
                );
                parts.push(tokio::select!{r=session.text(&segment.url)=>r?,_=cancel.cancelled()=>anyhow::bail!("CANCELLED")});
            }
            parts
        } else {
            vec![text]
        };
        let body = video_media::subtitles::merge(&parts)?;
        tokio::fs::write(&path, body).await?;
        Ok(path)
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.closing
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.notify.notify_one();
        let ids = self
            .jobs
            .read()
            .await
            .values()
            .filter(|j| {
                ["queued", "downloading", "muxing", "verifying"].contains(&j.state.as_str())
            })
            .map(|j| j.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.update(&id, |j| {
                if ["queued", "downloading", "muxing", "verifying"].contains(&j.state.as_str()) {
                    j.state = "paused".into();
                    j.stage = "interrupted".into();
                    j.speed = 0.;
                    j.eta = None;
                }
            })
            .await?;
            if let Some(c) = self.controls.lock().await.get(&id) {
                c.cancel();
            }
        }
        while !self.controls.lock().await.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(())
    }
}
async fn hash_file(path: &Path) -> Result<(u64, String)> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = vec![0; 128 * 1024];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hash.update(&buffer[..n]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}
pub fn redact(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            if word.contains("http://") || word.contains("https://") {
                "[URL đã ẩn]".to_string()
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
