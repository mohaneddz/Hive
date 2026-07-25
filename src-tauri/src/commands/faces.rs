use crate::ai::face::FaceModel;
use crate::commands::media::row_to_media_item;
use crate::jobs;
use crate::models::{MediaItem, PersonSummary};
use crate::state::AppState;
use rusqlite::{params, Connection};
use tauri::{AppHandle, State};

/// Cosine-similarity threshold above which two ArcFace embeddings are considered the same
/// person. Tuned conservatively (favoring more, smaller clusters over false merges) since
/// wrongly merging two different people is more disruptive to fix than splitting one person
/// into two clusters that get merged later.
const SAME_PERSON_THRESHOLD: f32 = 0.5;

#[tauri::command]
pub async fn download_face_models(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app_data_dir = state.app_data_dir.clone();
    let db_path = state.db_path.clone();
    let ai = state.ai.clone();

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "download_face_models").map_err(|e| e.to_string())?;

    let app_for_progress = app.clone();
    let db_path_for_progress = db_path.clone();
    let job_id_for_progress = job_id.clone();
    let result = crate::ai::model_manager::ensure_face_models(&app_data_dir, move |done, total| {
        if let Ok(conn) = crate::db::open(&db_path_for_progress) {
            jobs::emit_progress(
                &app_for_progress,
                &conn,
                &job_id_for_progress,
                "download_face_models",
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
            jobs::emit_progress(&app, &conn, &job_id, "download_face_models", "completed", 1, 1, None);
            match FaceModel::load(&crate::ai::model_manager::face_dir(&app_data_dir)) {
                Ok(model) => {
                    *ai.face.lock().unwrap() = Some(model);
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

fn ensure_face_loaded(state: &State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.ai.face.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    if !crate::ai::model_manager::face_models_ready(&state.app_data_dir) {
        return Err("Face models are not downloaded yet".to_string());
    }
    let model = FaceModel::load(&crate::ai::model_manager::face_dir(&state.app_data_dir))
        .map_err(|e| e.to_string())?;
    *guard = Some(model);
    Ok(())
}

fn crop_dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    app_data_dir.join("face_crops")
}

/// Finds the best-matching existing face (by cosine similarity) across the whole library and
/// returns its person_id if the match clears `SAME_PERSON_THRESHOLD`; brute-force over all
/// faces, fine at personal-library scale.
fn find_matching_person(conn: &Connection, embedding: &[f32]) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT person_id, embedding FROM faces WHERE person_id IS NOT NULL")?;
    let rows: Vec<(String, Vec<u8>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut best: Option<(String, f32)> = None;
    for (person_id, bytes) in rows {
        let other = crate::ai::clip::bytes_to_vector(&bytes);
        let score = crate::ai::face::cosine_similarity(embedding, &other);
        if best.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
            best = Some((person_id, score));
        }
    }

    Ok(best.filter(|(_, score)| *score >= SAME_PERSON_THRESHOLD).map(|(id, _)| id))
}

/// Detects, embeds, and clusters faces in a single already-indexed image if the face model is
/// loaded. Silently does nothing otherwise — the "detect as we go" hook called from indexing
/// and the file watcher, mirroring `try_embed_image`/`try_extract_ocr_text`.
pub fn try_detect_faces(
    ai: &crate::ai::AiState,
    app_data_dir: &std::path::Path,
    conn: &Connection,
    media_id: &str,
    path: &std::path::Path,
) {
    let mut guard = ai.face.lock().unwrap();
    let Some(model) = guard.as_mut() else { return };
    let Ok(faces) = model.detect_faces(path) else { return };

    let crop_dir = crop_dir(app_data_dir);
    let _ = std::fs::create_dir_all(&crop_dir);
    let Ok(img) = image::open(path) else { return };

    for face in faces {
        let person_id = match find_matching_person(conn, &face.embedding) {
            Ok(Some(id)) => id,
            _ => {
                let id = uuid::Uuid::new_v4().to_string();
                let now = chrono::Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "INSERT INTO people (id, name, created_at) VALUES (?1, NULL, ?2)",
                    params![id, now],
                );
                id
            }
        };

        let face_id = uuid::Uuid::new_v4().to_string();
        let (x0, y0, x1, y1) = face.bbox;
        let crop_path = crop_dir.join(format!("{face_id}.jpg"));
        if let Ok(cropped) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            img.crop_imm(x0, y0, x1 - x0, y1 - y0)
        })) {
            let _ = cropped
                .resize_to_fill(160, 160, image::imageops::FilterType::Triangle)
                .to_rgb8()
                .save_with_format(&crop_path, image::ImageFormat::Jpeg);
        }

        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO faces (id, media_id, person_id, x0, y0, x1, y1, embedding, crop_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                face_id,
                media_id,
                person_id,
                x0,
                y0,
                x1,
                y1,
                crate::ai::clip::vector_to_bytes(&face.embedding),
                crop_path.to_string_lossy(),
                now,
            ],
        );
    }

    let _ = conn.execute(
        "UPDATE media_items SET face_scanned = 1 WHERE id = ?1",
        params![media_id],
    );
}

#[tauri::command]
pub async fn backfill_faces(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    ensure_face_loaded(&state)?;

    let db_path = state.db_path.clone();
    let app_data_dir = state.app_data_dir.clone();
    let ai = state.ai.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "face_backfill").map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT id, path FROM media_items
                 WHERE media_type = 'image' AND is_trashed = 0 AND face_scanned = 0",
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
            if cancelled_jobs.lock().unwrap().remove(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(&app, &conn, &job_id, "face_backfill", "cancelled", i as i64, total, None);
                return Ok(());
            }
            try_detect_faces(&ai, &app_data_dir, &conn, media_id, std::path::Path::new(path));
            jobs::emit_progress(&app, &conn, &job_id, "face_backfill", "running", i as i64 + 1, total, None);
        }

        jobs::emit_progress(&app, &conn, &job_id, "face_backfill", "completed", total, total, None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_people(state: State<'_, AppState>) -> Result<Vec<PersonSummary>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, COUNT(f.id) as face_count, MIN(f.id) as cover_face_id
             FROM people p JOIN faces f ON f.person_id = p.id
             GROUP BY p.id
             ORDER BY face_count DESC",
        )
        .map_err(|e| e.to_string())?;
    let people = stmt
        .query_map([], |r| {
            Ok(PersonSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                face_count: r.get(2)?,
                cover_face_id: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(people)
}

#[tauri::command]
pub fn rename_person(state: State<'_, AppState>, person_id: String, name: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let trimmed = name.trim();
    let value: Option<&str> = if trimmed.is_empty() { None } else { Some(trimmed) };
    conn.execute("UPDATE people SET name = ?1 WHERE id = ?2", params![value, person_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn merge_people(state: State<'_, AppState>, source_id: String, target_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE faces SET person_id = ?1 WHERE person_id = ?2",
        params![target_id, source_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM people WHERE id = ?1", params![source_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_person_media(state: State<'_, AppState>, person_id: String) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT m.id FROM media_items m
             JOIN faces f ON f.media_id = m.id
             WHERE f.person_id = ?1 AND m.is_trashed = 0
             ORDER BY COALESCE(m.taken_at, m.created_at) DESC",
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![person_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_face_crop_bytes(state: State<'_, AppState>, face_id: String) -> Result<tauri::ipc::Response, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row("SELECT crop_path FROM faces WHERE id = ?1", params![face_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}
