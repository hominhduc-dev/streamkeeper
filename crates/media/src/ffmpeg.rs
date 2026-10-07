use anyhow::{ensure, Context, Result};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}
pub async fn execute(
    binary: &Path,
    args: &[String],
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    let mut command = Command::new(binary);
    command.args(args).kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Không khởi chạy được công cụ media")?;
    let result = tokio::select! {r=child.wait_with_output()=>r?, _=cancel.cancelled()=>anyhow::bail!("CANCELLED")};
    ensure!(
        result.status.success(),
        "Công cụ media thất bại: {}",
        String::from_utf8_lossy(&result.stderr)
            .chars()
            .take(1600)
            .collect::<String>()
    );
    Ok(result.stdout)
}
/// Retry timestamp errors using FFmpeg's muxer correction, then let the caller
/// validate the complete output before publishing it.
pub async fn mux(
    binary: &Path,
    args: &[String],
    output: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    match execute(binary, args, cancel).await {
        Ok(_) => Ok(()),
        Err(error) => {
            let message = error.to_string();
            if cancel.is_cancelled()
                || !(message.contains("Non-monotonic DTS")
                    || message.contains("Non-monotonous DTS"))
            {
                return Err(error);
            }
            tokio::fs::remove_file(output).await?;
            let corrected: Vec<String> = args
                .iter()
                .filter(|arg| arg.as_str() != "-xerror")
                .cloned()
                .collect();
            execute(binary, &corrected, cancel).await?;
            Ok(())
        }
    }
}

pub async fn probe(
    tools: &Tools,
    file: &Path,
    cancel: &CancellationToken,
) -> Result<serde_json::Value> {
    let bytes = execute(
        &tools.ffprobe,
        &[
            "-v".into(),
            "error".into(),
            "-show_format".into(),
            "-show_streams".into(),
            "-of".into(),
            "json".into(),
            file.to_string_lossy().into(),
        ],
        cancel,
    )
    .await?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub async fn validate(
    tools: &Tools,
    file: &Path,
    expected: f64,
    cancel: &CancellationToken,
) -> Result<()> {
    let value = probe(tools, file, cancel).await?;
    let duration = value["format"]["duration"]
        .as_str()
        .context("Thiếu thời lượng output")?
        .parse::<f64>()?;
    ensure!(
        (duration - expected).abs() <= 1.,
        "Thời lượng output {duration:.2}s lệch nguồn {expected:.2}s"
    );
    let streams = value["streams"].as_array().context("Thiếu tracks")?;
    ensure!(
        streams.iter().any(|s| s["codec_type"] == "video")
            && streams.iter().any(|s| s["codec_type"] == "audio"),
        "Output thiếu hình hoặc tiếng"
    );
    let path = file.to_string_lossy().to_string();
    execute(
        &tools.ffmpeg,
        &[
            "-v", "error", "-xerror", "-i", &path, "-map", "0:v:0", "-map", "0:a:0", "-c", "copy",
            "-f", "null", "-",
        ]
        .map(str::to_string),
        cancel,
    )
    .await?;
    for time in [0., duration / 2., (duration - 3.).max(0.)] {
        execute(
            &tools.ffmpeg,
            &[
                "-v",
                "error",
                "-xerror",
                "-ss",
                &format!("{time}"),
                "-i",
                &path,
                "-t",
                "2",
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-f",
                "null",
                "-",
            ]
            .map(str::to_string),
            cancel,
        )
        .await?;
    }
    Ok(())
}
pub async fn version(binary: &Path) -> String {
    let cancel = CancellationToken::new();
    match tokio::time::timeout(
        Duration::from_secs(8),
        execute(binary, &["-version".into()], &cancel),
    )
    .await
    {
        Ok(Ok(b)) => String::from_utf8_lossy(&b)
            .lines()
            .next()
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}
