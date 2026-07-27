//! Image captioning commands: download the ViT-GPT2 model, generate captions in
//! bulk, and retrieve captions for individual images.

use crate::ai::captioning::CaptionModel;
use crate::ai::model_manager;
use crate::jobs;
use crate::state::AppState;
use rusqlite::params;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn download_caption_model(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app_data_dir = state.app_data_dir.clone();
    model_manager::ensure_caption_models(&app_data_dir, move |done, total| {
        let _ = app.emit("caption:download:progress", (done, total));
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_captions(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut guard = state.ai.caption.lock().unwrap();
        if guard.is_none() {
            if !model_manager::caption_models_ready(&state.app_data_dir) {
                return Err("Caption model is not downloaded yet".to_string());
            }
            let model = CaptionModel::load(&model_manager::caption_dir(&state.app_data_dir))
                .map_err(|e| e.to_string())?;
            *guard = Some(model);
        }
    }

    let db_path = state.db_path.clone();
    let ai = state.ai.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "caption_backfill").map_err(|e| e.to_string())?;

        // Find images without a caption.
        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.path FROM media_items m
                 WHERE m.media_type = 'image' AND m.is_trashed = 0
                   AND NOT EXISTS (SELECT 1 FROM captions c WHERE c.media_id = m.id)",
            )
            .map_err(|e| e.to_string())?;
        let pending: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let total = pending.len() as i64;
        // A model that cannot run fails on every image identically. Swallowing
        // that is how a decoder missing 24 of its inputs went unnoticed: the bar
        // reached 100% and the library gained nothing.
        let mut first_error: Option<String> = None;
        let mut written = 0i64;

        for (i, (media_id, path)) in pending.iter().enumerate() {
            if cancelled_jobs.lock().unwrap().remove(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(
                    &app, &conn, &job_id, "caption_backfill", "cancelled",
                    i as i64, total, None,
                );
                return Ok(());
            }

            let caption = {
                let mut guard = ai.caption.lock().unwrap();
                let model = guard.as_mut().unwrap();
                model.caption(std::path::Path::new(path))
            };

            match caption {
                Ok(text) => {
                    let now = chrono::Utc::now().to_rfc3339();
                    let _ = conn.execute(
                        "INSERT INTO captions (media_id, text, model, created_at)
                         VALUES (?1, ?2, 'vit-gpt2', ?3)
                         ON CONFLICT(media_id) DO UPDATE SET
                            text = excluded.text, created_at = excluded.created_at",
                        params![media_id, text, now],
                    );
                    written += 1;
                }
                Err(cause) => {
                    first_error.get_or_insert_with(|| cause.to_string());
                }
            }

            jobs::emit_progress(
                &app, &conn, &job_id, "caption_backfill", "running",
                i as i64 + 1, total, None,
            );
        }

        // One unreadable photo among hundreds is not worth failing the run; not a
        // single caption written means the model itself is the problem, and the
        // user needs to be told rather than left with an empty success.
        if let Some(error) = first_error.filter(|_| written == 0 && total > 0) {
            jobs::emit_progress(
                &app, &conn, &job_id, "caption_backfill", "failed",
                total, total, Some(error.clone()),
            );
            return Err(format!("No caption could be generated: {error}"));
        }

        jobs::emit_progress(&app, &conn, &job_id, "caption_backfill", "completed", total, total, None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Retrieves the caption for a single media item.
#[tauri::command]
pub fn get_caption(state: State<'_, AppState>, media_id: String) -> Result<Option<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let text: Option<String> = conn
        .query_row(
            "SELECT text FROM captions WHERE media_id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .ok();
    Ok(text)
}
