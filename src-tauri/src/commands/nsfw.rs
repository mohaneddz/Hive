//! Sensitive content detection commands: download the NSFW model, score images,
//! and auto-hide those above a threshold.

use crate::ai::model_manager;
use crate::ai::nsfw::NsfwModel;
use crate::jobs;
use crate::state::AppState;
use rusqlite::params;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn download_nsfw_model(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app_data_dir = state.app_data_dir.clone();
    model_manager::ensure_nsfw_models(&app_data_dir, move |done, total| {
        let _ = app.emit("nsfw:download:progress", (done, total));
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_nsfw(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut guard = state.ai.nsfw.lock().unwrap();
        if guard.is_none() {
            if !model_manager::nsfw_models_ready(&state.app_data_dir) {
                return Err("NSFW model is not downloaded yet".to_string());
            }
            let model = NsfwModel::load(&model_manager::nsfw_dir(&state.app_data_dir))
                .map_err(|e| e.to_string())?;
            *guard = Some(model);
        }
    }

    let db_path = state.db_path.clone();
    let ai = state.ai.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "nsfw_backfill").map_err(|e| e.to_string())?;

        // Read once: the policy must not change halfway through a run, or the
        // library ends up sorted by two different rules.
        let policy = crate::commands::preferences::nsfw_policy(&conn);

        let mut stmt = conn
            .prepare(
                "SELECT id, path FROM media_items
                 WHERE media_type = 'image' AND is_trashed = 0 AND nsfw_score IS NULL",
            )
            .map_err(|e| e.to_string())?;
        let pending: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let total = pending.len() as i64;
        // A model that cannot run fails identically on every image. Reporting
        // nothing would leave the user with a finished bar and an unscored library.
        let mut first_error: Option<String> = None;
        let mut scored = 0i64;

        for (i, (media_id, path)) in pending.iter().enumerate() {
            if cancelled_jobs.lock().unwrap().remove(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(
                    &app, &conn, &job_id, "nsfw_backfill", "cancelled",
                    i as i64, total, None,
                );
                return Ok(());
            }

            let score = {
                let mut guard = ai.nsfw.lock().unwrap();
                let model = guard.as_mut().unwrap();
                model.score(std::path::Path::new(path))
            };

            match score {
                Ok(score) => {
                    let _ = conn.execute(
                        "UPDATE media_items SET nsfw_score = ?1 WHERE id = ?2",
                        params![score, media_id],
                    );
                    // Only when the user asked for it. The score alone always
                    // covers the photo in the grid, which is reversible; filing it
                    // away is not something a guess should do uninvited.
                    if policy.auto_hide && score >= policy.threshold {
                        let _ = conn.execute(
                            "UPDATE media_items SET is_hidden = 1 WHERE id = ?1 AND is_hidden = 0",
                            params![media_id],
                        );
                    }
                    scored += 1;
                }
                Err(cause) => {
                    first_error.get_or_insert_with(|| cause.to_string());
                }
            }

            jobs::emit_progress(
                &app, &conn, &job_id, "nsfw_backfill", "running",
                i as i64 + 1, total, None,
            );
        }

        // One unreadable file is not worth failing the run; nothing scored at all
        // means the model is the problem and saying so beats a silent success.
        if let Some(error) = first_error.filter(|_| scored == 0 && total > 0) {
            jobs::emit_progress(
                &app, &conn, &job_id, "nsfw_backfill", "failed",
                total, total, Some(error.clone()),
            );
            return Err(format!("No image could be scored: {error}"));
        }

        jobs::emit_progress(&app, &conn, &job_id, "nsfw_backfill", "completed", total, total, None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
