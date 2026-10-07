use anyhow::Result;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::path::Path;
use video_domain::{Job, Settings};
#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}
impl Storage {
    pub async fn open(path: &Path) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(10));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }
    pub async fn save(&self, job: &Job) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO jobs VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload, updated=excluded.updated")
            .bind(&job.id).bind(serde_json::to_string(job)?).bind(job.updated_at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO job_sources VALUES(?,?) ON CONFLICT(job_id) DO UPDATE SET source=excluded.source")
            .bind(&job.id).bind(&job.source).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn jobs(&self) -> Result<Vec<Job>> {
        let rows = sqlx::query("SELECT j.payload,s.source FROM jobs j JOIN job_sources s ON j.id=s.job_id ORDER BY j.updated DESC").fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|r| {
                let mut j: Job = serde_json::from_str(r.get("payload"))?;
                j.source = r.get("source");
                Ok(j)
            })
            .collect()
    }
    pub async fn settings(&self) -> Result<Option<Settings>> {
        let row = sqlx::query("SELECT payload FROM settings WHERE id=1")
            .fetch_optional(&self.pool)
            .await?;
        row.map(|r| serde_json::from_str(r.get("payload")).map_err(Into::into))
            .transpose()
    }
    pub async fn set_settings(&self, value: &Settings) -> Result<()> {
        sqlx::query("INSERT INTO settings VALUES(1,?) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload")
            .bind(serde_json::to_string(value)?).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn checkpoint(&self, job: &str, id: &str, bytes: u64, checksum: &str) -> Result<()> {
        sqlx::query("INSERT INTO segments VALUES(?,?,?,?) ON CONFLICT(job_id,stable_id) DO UPDATE SET bytes=excluded.bytes,checksum=excluded.checksum")
            .bind(job).bind(id).bind(bytes as i64).bind(checksum).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn segment(&self, job: &str, id: &str) -> Result<Option<(u64, String)>> {
        Ok(
            sqlx::query("SELECT bytes,checksum FROM segments WHERE job_id=? AND stable_id=?")
                .bind(job)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .map(|r| (r.get::<i64, _>("bytes") as u64, r.get("checksum"))),
        )
    }
}
