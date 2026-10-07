#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};
use video_domain::*;
use video_engine::Engine;
type AppEngine = Arc<Engine>;
fn message(e: anyhow::Error) -> String {
    video_engine::redact(&e.to_string())
}
#[tauri::command]
async fn inspect_source(state: State<'_, AppEngine>, source: String) -> Result<Inspection, String> {
    state.inspect(source).await.map_err(message)
}
#[tauri::command]
async fn create_download(
    state: State<'_, AppEngine>,
    options: DownloadOptions,
) -> Result<Job, String> {
    state.create(options).await.map_err(message)
}
#[tauri::command]
async fn list_downloads(state: State<'_, AppEngine>) -> Result<Vec<Job>, String> {
    Ok(state.list().await)
}
#[tauri::command]
async fn get_download(state: State<'_, AppEngine>, id: String) -> Result<Job, String> {
    state.get(&id).await.map_err(message)
}
#[tauri::command]
async fn pause_download(state: State<'_, AppEngine>, id: String) -> Result<(), String> {
    state.pause(&id).await.map_err(message)
}
#[tauri::command]
async fn resume_download(state: State<'_, AppEngine>, id: String) -> Result<(), String> {
    state.resume(&id).await.map_err(message)
}
#[tauri::command]
async fn retry_download(state: State<'_, AppEngine>, id: String) -> Result<(), String> {
    state.resume(&id).await.map_err(message)
}
#[tauri::command]
async fn cancel_download(
    state: State<'_, AppEngine>,
    id: String,
    keep_cache: bool,
) -> Result<(), String> {
    state.cancel(&id, keep_cache).await.map_err(message)
}
#[tauri::command]
async fn cleanup_job_cache(state: State<'_, AppEngine>, id: String) -> Result<(), String> {
    state.cleanup(&id).await.map_err(message)
}
#[tauri::command]
async fn get_settings(state: State<'_, AppEngine>) -> Result<Settings, String> {
    Ok(state.settings().await)
}
#[tauri::command]
async fn update_settings(state: State<'_, AppEngine>, settings: Settings) -> Result<(), String> {
    state.set_settings(settings).await.map_err(message)
}
#[tauri::command]
async fn runtime_status(state: State<'_, AppEngine>) -> Result<RuntimeStatus, String> {
    let a = video_media::ffmpeg::version(&state.tools.ffmpeg).await;
    let b = video_media::ffmpeg::version(&state.tools.ffprobe).await;
    Ok(RuntimeStatus {
        ffmpeg: !a.is_empty(),
        ffprobe: !b.is_empty(),
        ffmpeg_version: a,
        data_dir: state.data_dir.to_string_lossy().into(),
    })
}
#[tauri::command]
async fn open_artifact(
    state: State<'_, AppEngine>,
    id: String,
    folder: bool,
) -> Result<(), String> {
    let job = state.get(&id).await.map_err(message)?;
    let path = PathBuf::from(job.output.ok_or("Không có đường dẫn output")?);
    if folder {
        let parent = path.parent().ok_or("Không có thư mục")?;
        std::process::Command::new("explorer.exe")
            .arg(parent)
            .spawn()
            .map_err(|_| "Không mở được thư mục")?;
    } else {
        if !path.exists() {
            return Err("File đã bị di chuyển hoặc xóa".into());
        }
        std::process::Command::new("explorer.exe")
            .arg(&path)
            .spawn()
            .map_err(|_| "Không mở được file")?;
    }
    Ok(())
}
#[tauri::command]
async fn shutdown_app(state: State<'_, AppEngine>, app: tauri::AppHandle) -> Result<(), String> {
    state.shutdown().await.map_err(message)?;
    app.exit(0);
    Ok(())
}
#[tauri::command]
async fn export_diagnostics(
    state: State<'_, AppEngine>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let jobs = state.list().await;
    let path = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join("streamkeeper-diagnostics.json");
    let safe = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"jobs":jobs.iter().map(|j|serde_json::json!({"id":j.id,"state":j.state,"stage":j.stage,"error":j.error,"errorCode":j.error_code,"segments":j.completed_segments,"totalSegments":j.total_segments})).collect::<Vec<_>>()});
    tokio::fs::write(
        &path,
        serde_json::to_vec_pretty(&safe).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}
fn tools() -> video_media::ffmpeg::Tools {
    let parent = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let locate = |name: &str| {
        let packaged = parent.join(format!("{name}.exe"));
        if packaged.exists() {
            packaged
        } else {
            dev.join(format!("{name}-x86_64-pc-windows-msvc.exe"))
        }
    };
    video_media::ffmpeg::Tools {
        ffmpeg: locate("ffmpeg"),
        ffprobe: locate("ffprobe"),
    }
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            inspect_source,
            create_download,
            list_downloads,
            get_download,
            pause_download,
            resume_download,
            retry_download,
            cancel_download,
            cleanup_job_cache,
            get_settings,
            update_settings,
            runtime_status,
            open_artifact,
            shutdown_app,
            export_diagnostics,
            list_movies,
            mark_movie_watched,
            rename_movie,
            delete_movie,
            movie_poster
        ])
        .setup(|app| {
            let data = std::env::var_os("STREAMKEEPER_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_local_data_dir()?);
            let output = app.path().download_dir()?.join("Streamkeeper");
            let engine = tauri::async_runtime::block_on(Engine::open(data, output, tools()))?;
            let mut receiver = engine.events.subscribe();
            let handle = app.handle().clone();
            app.manage(engine);
            tauri::async_runtime::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(job) => {
                            let _ = handle.emit("download-update", job);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let _ = handle.emit("downloads-reconcile", ());
                        }
                        Err(_) => break,
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .run(tauri::generate_context!())
        .expect("Không khởi chạy được Streamkeeper");
}
#[tauri::command]
async fn list_movies(state: State<'_, AppEngine>) -> Result<Vec<Movie>, String> {
    state.movies().await.map_err(message)
}
#[tauri::command]
async fn mark_movie_watched(
    state: State<'_, AppEngine>,
    id: String,
    watched: bool,
) -> Result<(), String> {
    state.mark_watched(&id, watched).await.map_err(message)
}
#[tauri::command]
async fn rename_movie(state: State<'_, AppEngine>, id: String, name: String) -> Result<(), String> {
    state.rename_movie(&id, &name).await.map_err(message)
}
#[tauri::command]
async fn delete_movie(state: State<'_, AppEngine>, id: String) -> Result<(), String> {
    state.delete_movie(&id).await.map_err(message)
}
#[tauri::command]
async fn movie_poster(state: State<'_, AppEngine>, id: String) -> Result<String, String> {
    state.movie_poster(&id).await.map_err(message)
}
