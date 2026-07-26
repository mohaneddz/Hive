use crate::ai::llm::ChatModel;
use crate::jobs;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub answer: String,
    pub media_ids: Vec<String>,
}

const SYSTEM_PROMPT: &str = "You are Hive, a local assistant for someone's personal photo \
library. You are given a short list of photos (filename, date, camera, and any text found in \
the photo) that were retrieved for the user's question. Answer using only what's in that list \
— if it doesn't contain the answer, say so plainly rather than guessing. Keep replies to a \
few sentences, in a warm and direct tone, and don't repeat the raw list back verbatim.";

#[tauri::command]
pub async fn download_llm_model(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app_data_dir = state.app_data_dir.clone();
    let db_path = state.db_path.clone();
    let ai = state.ai.clone();

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "download_llm_model").map_err(|e| e.to_string())?;

    let app_for_progress = app.clone();
    let db_path_for_progress = db_path.clone();
    let job_id_for_progress = job_id.clone();
    let result = crate::ai::model_manager::ensure_llm_models(&app_data_dir, move |done, total| {
        if let Ok(conn) = crate::db::open(&db_path_for_progress) {
            jobs::emit_progress(
                &app_for_progress,
                &conn,
                &job_id_for_progress,
                "download_llm_model",
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
            jobs::emit_progress(&app, &conn, &job_id, "download_llm_model", "completed", 1, 1, None);
            match ChatModel::load(&crate::ai::model_manager::llm_dir(&app_data_dir)) {
                Ok(model) => {
                    *ai.llm.lock().unwrap() = Some(model);
                    Ok(())
                }
                Err(e) => Err(format!("Model downloaded but failed to load: {e}")),
            }
        }
        Err(e) => {
            jobs::fail_job(&conn, &job_id, &e.to_string());
            Err(e.to_string())
        }
    }
}

fn ensure_llm_loaded(state: &State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.ai.llm.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    if !crate::ai::model_manager::llm_models_ready(&state.app_data_dir) {
        return Err("The local chat model is not downloaded yet".to_string());
    }
    let model = ChatModel::load(&crate::ai::model_manager::llm_dir(&state.app_data_dir))
        .map_err(|e| e.to_string())?;
    *guard = Some(model);
    Ok(())
}

/// Retrieves the media ids most relevant to `query`, preferring CLIP semantic search when the
/// model is loaded and falling back to the FTS5 exact-text index otherwise.
fn retrieve_context_ids(state: &State<'_, AppState>, query: &str, limit: i64) -> Result<Vec<String>, String> {
    let clip_loaded = state.ai.clip.lock().unwrap().is_some();
    if clip_loaded {
        if let Ok(items) = crate::commands::ai::semantic_search(state.clone(), query.to_string(), limit) {
            if !items.is_empty() {
                return Ok(items.into_iter().map(|m| m.id).collect());
            }
        }
    }
    let items = crate::commands::media::search_media(state.clone(), query.to_string(), limit)?;
    Ok(items.into_iter().map(|m| m.id).collect())
}

struct ContextRow {
    filename: String,
    taken_at: Option<String>,
    camera_model: Option<String>,
    ocr_text: Option<String>,
}

fn load_context(conn: &rusqlite::Connection, media_ids: &[String]) -> rusqlite::Result<Vec<ContextRow>> {
    let mut rows = Vec::with_capacity(media_ids.len());
    for id in media_ids {
        let row = conn.query_row(
            "SELECT m.id, m.filename, COALESCE(m.taken_at_override, m.taken_at), e.camera_model, f.ocr_text
             FROM media_items m
             LEFT JOIN exif_data e ON e.media_id = m.id
             LEFT JOIN media_fts f ON f.media_id = m.id
             WHERE m.id = ?1",
            rusqlite::params![id],
            |r| {
                Ok(ContextRow {
                    filename: r.get(1)?,
                    taken_at: r.get(2)?,
                    camera_model: r.get(3)?,
                    ocr_text: r.get(4)?,
                })
            },
        )?;
        rows.push(row);
    }
    Ok(rows)
}

fn format_context(rows: &[ContextRow]) -> String {
    if rows.is_empty() {
        return "(no matching photos were found in the library)".to_string();
    }
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let date = row.taken_at.as_deref().unwrap_or("unknown date");
            let camera = row.camera_model.as_deref().unwrap_or("unknown camera");
            let text = row
                .ocr_text
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(|t| format!(" — visible text: \"{}\"", t.chars().take(200).collect::<String>()))
                .unwrap_or_default();
            format!("{}. {} (taken {}, {}){}", i + 1, row.filename, date, camera, text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tauri::command]
pub async fn gallery_chat(app: AppHandle, state: State<'_, AppState>, message: String) -> Result<ChatResponse, String> {
    ensure_llm_loaded(&state)?;

    let media_ids = retrieve_context_ids(&state, &message, 8)?;
    let context = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        format_context(&load_context(&conn, &media_ids).map_err(|e| e.to_string())?)
    };

    let ai = state.ai.clone();
    let db_path = state.db_path.clone();
    let user_prompt = format!("Photos found for this question:\n{context}\n\nQuestion: {message}");

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "gallery_chat").map_err(|e| e.to_string())?;
    jobs::emit_progress(&app, &conn, &job_id, "gallery_chat", "running", 0, 1, None);

    let answer = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let mut guard = ai.llm.lock().unwrap();
        let model = guard.as_mut().ok_or("chat model not loaded")?;
        model.chat(SYSTEM_PROMPT, &user_prompt).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;

    match &answer {
        Ok(_) => jobs::emit_progress(&app, &conn, &job_id, "gallery_chat", "completed", 1, 1, None),
        Err(e) => jobs::fail_job(&conn, &job_id, e),
    }

    Ok(ChatResponse { answer: answer?, media_ids })
}
