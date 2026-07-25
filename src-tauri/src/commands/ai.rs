use crate::ai::{clip::ClipModel, model_manager};
use crate::commands::media::row_to_media_item;
use crate::jobs;
use crate::models::MediaItem;
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

const CLIP_MODEL_NAME: &str = "clip-vit-base-patch32";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    pub models_ready: bool,
    pub model_loaded: bool,
    pub embedded_count: i64,
    pub eligible_count: i64,
}

#[tauri::command]
pub fn get_ai_status(state: State<'_, AppState>) -> Result<AiStatus, String> {
    let models_ready = model_manager::clip_models_ready(&state.app_data_dir);
    let model_loaded = state.ai.clip.lock().unwrap().is_some();

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let embedded_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM embeddings", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let eligible_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM media_items WHERE media_type = 'image' AND is_trashed = 0",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(AiStatus {
        models_ready,
        model_loaded,
        embedded_count,
        eligible_count,
    })
}

#[tauri::command]
pub async fn download_ai_models(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app_data_dir = state.app_data_dir.clone();
    let db_path = state.db_path.clone();
    let ai = state.ai.clone();

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "download_models").map_err(|e| e.to_string())?;

    let app_for_progress = app.clone();
    let db_path_for_progress = db_path.clone();
    let job_id_for_progress = job_id.clone();
    let result = model_manager::ensure_clip_models(&app_data_dir, move |done, total| {
        if let Ok(conn) = crate::db::open(&db_path_for_progress) {
            jobs::emit_progress(
                &app_for_progress,
                &conn,
                &job_id_for_progress,
                "download_models",
                "running",
                done as i64,
                total as i64,
                None,
            );
        }
    })
    .await;

    match result {
        Ok(()) => {
            jobs::emit_progress(&app, &conn, &job_id, "download_models", "completed", 1, 1, None);
            match ClipModel::load(&model_manager::clip_dir(&app_data_dir)) {
                Ok(model) => {
                    *ai.clip.lock().unwrap() = Some(model);
                    Ok(())
                }
                Err(e) => Err(format!("Models downloaded but failed to load: {e}")),
            }
        }
        Err(e) => {
            jobs::fail_job(&conn, &job_id, &e.to_string());
            Err(e.to_string())
        }
    }
}

fn ensure_loaded(state: &State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.ai.clip.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    if !model_manager::clip_models_ready(&state.app_data_dir) {
        return Err("AI models are not downloaded yet".to_string());
    }
    let model = ClipModel::load(&model_manager::clip_dir(&state.app_data_dir)).map_err(|e| e.to_string())?;
    *guard = Some(model);
    Ok(())
}

#[tauri::command]
pub fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<Vec<MediaItem>, String> {
    ensure_loaded(&state)?;

    let query_vec = {
        let mut guard = state.ai.clip.lock().unwrap();
        guard.as_mut().unwrap().embed_text(&query).map_err(|e| e.to_string())?
    };

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT e.media_id, e.vector FROM embeddings e
             JOIN media_items m ON m.id = e.media_id
             WHERE m.is_trashed = 0",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, Vec<u8>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut scored: Vec<(String, f32)> = rows
        .into_iter()
        .map(|(id, bytes)| {
            let vec = crate::ai::clip::bytes_to_vector(&bytes);
            let score = crate::ai::clip::cosine_similarity(&query_vec, &vec);
            (id, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(limit.max(0) as usize);

    scored
        .into_iter()
        .map(|(id, _)| row_to_media_item(&conn, &id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn store_embedding(conn: &Connection, media_id: &str, vector: &[f32]) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO embeddings (media_id, model, dim, vector, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(media_id) DO UPDATE SET
            model = excluded.model, dim = excluded.dim, vector = excluded.vector,
            created_at = excluded.created_at",
        params![
            media_id,
            CLIP_MODEL_NAME,
            vector.len() as i64,
            crate::ai::clip::vector_to_bytes(vector),
            now,
        ],
    )?;
    Ok(())
}

/// Embeds a single already-indexed image if the CLIP model is loaded. Silently does nothing
/// otherwise — this is the "embed as we go" hook called from indexing and the file watcher.
pub fn try_embed_image(ai: &crate::ai::AiState, conn: &Connection, media_id: &str, path: &std::path::Path) {
    let mut guard = ai.clip.lock().unwrap();
    let Some(model) = guard.as_mut() else { return };
    if let Ok(vector) = model.embed_image(path) {
        let _ = store_embedding(conn, media_id, &vector);
    }
}

#[tauri::command]
pub async fn backfill_embeddings(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    ensure_loaded(&state)?;

    let db_path = state.db_path.clone();
    let ai = state.ai.clone();

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "embed_backfill").map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.path FROM media_items m
             LEFT JOIN embeddings e ON e.media_id = m.id
             WHERE m.media_type = 'image' AND m.is_trashed = 0 AND e.media_id IS NULL",
        )
        .map_err(|e| e.to_string())?;
    let pending: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let total = pending.len() as i64;
    for (i, (media_id, path)) in pending.iter().enumerate() {
        let mut guard = ai.clip.lock().unwrap();
        if let Some(model) = guard.as_mut() {
            if let Ok(vector) = model.embed_image(std::path::Path::new(path)) {
                let _ = store_embedding(&conn, media_id, &vector);
            }
        }
        drop(guard);

        jobs::emit_progress(
            &app,
            &conn,
            &job_id,
            "embed_backfill",
            "running",
            i as i64 + 1,
            total,
            None,
        );
    }

    jobs::emit_progress(&app, &conn, &job_id, "embed_backfill", "completed", total, total, None);
    Ok(())
}
