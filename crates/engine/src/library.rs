use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
impl Engine {
    pub async fn movies(&self) -> Result<Vec<Movie>> {
        let _guard = self.library_lock.lock().await;
        let mut movies = Vec::new();
        for job in self.list().await {
            if job.state != "completed" {
                continue;
            }
            let (watched, deleted) = self.storage.movie_flags(&job.id).await?;
            if deleted {
                continue;
            }
            if let Some(output) = job.output {
                movies.push(Movie {
                    id: job.id,
                    title: job.title,
                    missing: !Path::new(&output).is_file(),
                    output,
                    format: job.options.format,
                    duration: job.duration,
                    added_at: job.updated_at,
                    watched,
                });
            }
        }
        Ok(movies)
    }
    async fn library_job(&self, id: &str) -> Result<Job> {
        let job = self.get(id).await?;
        ensure!(
            job.state == "completed" && !self.storage.movie_flags(id).await?.1,
            "Phim không còn trong thư viện"
        );
        Ok(job)
    }
    pub async fn mark_watched(&self, id: &str, watched: bool) -> Result<()> {
        let _guard = self.library_lock.lock().await;
        self.library_job(id).await?;
        self.storage.set_movie_flags(id, watched, false).await
    }
    pub async fn rename_movie(&self, id: &str, name: &str) -> Result<()> {
        let _guard = self.library_lock.lock().await;
        ensure!(safe_filename(name), "Tên file không hợp lệ trên Windows");
        let job = self.library_job(id).await?;
        let old = PathBuf::from(job.output.as_ref().context("Thiếu output")?);
        let new = old.with_file_name(format!("{name}.{}", job.options.format));
        ensure!(old.is_file(), "File đã bị di chuyển hoặc xóa");
        if old == new {
            return Ok(());
        }
        ensure!(!new.exists(), "Tên file đã tồn tại");
        tokio::fs::hard_link(&old, &new)
            .await
            .context("Không đổi được tên file")?;
        if let Err(error) = tokio::fs::remove_file(&old).await {
            let _ = tokio::fs::remove_file(&new).await;
            return Err(error.into());
        }
        if let Err(error) = self
            .update(id, |j| {
                j.output = Some(new.to_string_lossy().into());
                j.options.filename = name.into();
                j.title = name.into();
            })
            .await
        {
            let _ = tokio::fs::rename(&new, &old).await;
            return Err(error);
        }
        Ok(())
    }
    pub async fn delete_movie(&self, id: &str) -> Result<()> {
        let _guard = self.library_lock.lock().await;
        let job = self.library_job(id).await?;
        let output = PathBuf::from(job.output.context("Thiếu output")?);
        match tokio::fs::remove_file(&output).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        };
        let (watched, _) = self.storage.movie_flags(id).await?;
        self.storage.set_movie_flags(id, watched, true).await?;
        let _ =
            tokio::fs::remove_file(self.data_dir.join("posters").join(format!("{id}.jpg"))).await;
        Ok(())
    }
    pub async fn movie_poster(&self, id: &str) -> Result<String> {
        let _guard = self.library_lock.lock().await;
        let job = self.library_job(id).await?;
        let dir = self.data_dir.join("posters");
        tokio::fs::create_dir_all(&dir).await?;
        let poster = dir.join(format!("{id}.jpg"));
        if !poster.exists() {
            let output = job.output.context("Thiếu output")?;
            ensure!(Path::new(&output).is_file(), "Không tìm thấy video");
            ffmpeg::execute(
                &self.tools.ffmpeg,
                &[
                    "-v".into(),
                    "error".into(),
                    "-nostdin".into(),
                    "-y".into(),
                    "-ss".into(),
                    (job.duration * 0.1).min(60.).to_string(),
                    "-i".into(),
                    output,
                    "-frames:v".into(),
                    "1".into(),
                    "-vf".into(),
                    "scale=480:-2".into(),
                    poster.to_string_lossy().into(),
                ],
                &CancellationToken::new(),
            )
            .await?;
        }
        Ok(format!(
            "data:image/jpeg;base64,{}",
            STANDARD.encode(tokio::fs::read(poster).await?)
        ))
    }
}
