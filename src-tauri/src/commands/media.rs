use crate::models::{ExifData, LibraryStats, MediaItem, MediaPage, PlaceCluster};
use crate::state::AppState;
use crate::{db, indexing, jobs, thumbnails};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, State};

pub(crate) fn row_to_media_item(conn: &Connection, id: &str) -> rusqlite::Result<MediaItem> {
    let mut item = conn.query_row(
        "SELECT id, folder_id, path, filename, hash, size, width, height, duration_ms,
                mime_type, media_type, taken_at, created_at, modified_at, indexed_at,
                is_favorite, is_trashed
         FROM media_items WHERE id = ?1",
        params![id],
        |r| {
            Ok(MediaItem {
                id: r.get(0)?,
                folder_id: r.get(1)?,
                path: r.get(2)?,
                filename: r.get(3)?,
                hash: r.get(4)?,
                size: r.get(5)?,
                width: r.get(6)?,
                height: r.get(7)?,
                duration_ms: r.get(8)?,
                mime_type: r.get(9)?,
                media_type: r.get(10)?,
                taken_at: r.get(11)?,
                created_at: r.get(12)?,
                modified_at: r.get(13)?,
                indexed_at: r.get(14)?,
                is_favorite: r.get::<_, i64>(15)? != 0,
                is_trashed: r.get::<_, i64>(16)? != 0,
                exif: None,
                thumbnail_path: None,
            })
        },
    )?;

    item.exif = conn
        .query_row(
            "SELECT camera_make, camera_model, lens, iso, f_number, exposure_time,
                    focal_length, gps_lat, gps_lon
             FROM exif_data WHERE media_id = ?1",
            params![id],
            |r| {
                Ok(ExifData {
                    camera_make: r.get(0)?,
                    camera_model: r.get(1)?,
                    lens: r.get(2)?,
                    iso: r.get(3)?,
                    f_number: r.get(4)?,
                    exposure_time: r.get(5)?,
                    focal_length: r.get(6)?,
                    gps_lat: r.get(7)?,
                    gps_lon: r.get(8)?,
                })
            },
        )
        .optional()?;

    item.thumbnail_path = conn
        .query_row(
            "SELECT path FROM thumbnails WHERE media_id = ?1 AND size = 'md'",
            params![id],
            |r| r.get(0),
        )
        .optional()?;

    Ok(item)
}

#[tauri::command]
pub fn scan_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<String, String> {
    let db_path = state.db_path.clone();
    let app_data_dir = state.app_data_dir.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();
    let ai = state.ai.clone();

    let folder_path: String = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT path FROM folders WHERE id = ?1", params![folder_id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?
    };

    let conn = db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "scan_folder").map_err(|e| e.to_string())?;
    let job_id_ret = job_id.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let Ok(conn) = db::open(&db_path) else { return };
        let files = indexing::walk_folder(std::path::Path::new(&folder_path));
        let total = files.len() as i64;

        jobs::emit_progress(&app, &conn, &job_id, "scan_folder", "running", 0, total, None);
        let _ = tauri::Emitter::emit(&app, "media:changed", &folder_id);

        // Indexed here as (media_id, path) so the AI enrichment pass below only has to touch
        // files this scan actually added/changed, not the whole library.
        let mut newly_indexed: Vec<(String, std::path::PathBuf)> = Vec::new();

        for (i, path) in files.iter().enumerate() {
            if cancelled_jobs.lock().unwrap().contains(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(
                    &app,
                    &conn,
                    &job_id,
                    "scan_folder",
                    "cancelled",
                    i as i64,
                    total,
                    Some("Cancelled by user".to_string()),
                );
                cancelled_jobs.lock().unwrap().remove(&job_id);
                let _ = tauri::Emitter::emit(&app, "media:changed", &folder_id);
                return;
            }

            match indexing::index_file(&conn, &folder_id, path) {
                Ok(Some(indexed)) => {
                    let _ = thumbnails::generate_for_image(
                        &conn,
                        &app_data_dir,
                        &indexed.item.id,
                        path,
                    );
                    newly_indexed.push((indexed.item.id, path.clone()));
                    let _ = tauri::Emitter::emit(&app, "media:changed", &folder_id);
                }
                Ok(None) => {}
                Err(e) => {
                    jobs::fail_job(&conn, &job_id, &e.to_string());
                }
            }

            jobs::emit_progress(
                &app,
                &conn,
                &job_id,
                "scan_folder",
                "running",
                i as i64 + 1,
                total,
                path.file_name().map(|n| n.to_string_lossy().to_string()),
            );
        }

        jobs::emit_progress(&app, &conn, &job_id, "scan_folder", "completed", total, total, None);
        let _ = tauri::Emitter::emit(&app, "media:changed", &folder_id);

        // AI enrichment (CLIP embeddings + OCR) runs as its own job *after* the scan reports
        // complete, so thumbnails show up immediately and indexing speed doesn't depend on
        // whether the (much slower) AI models happen to be loaded. Only runs for models the
        // user has already downloaded — no-op otherwise.
        let ai_ready = ai.clip.lock().unwrap().is_some() || ai.ocr.lock().unwrap().is_some();
        if ai_ready && !newly_indexed.is_empty() {
            let enrich_job = jobs::create_job(&conn, "enrich_media").unwrap_or_default();
            let enrich_total = newly_indexed.len() as i64;
            for (i, (media_id, path)) in newly_indexed.iter().enumerate() {
                if cancelled_jobs.lock().unwrap().remove(&enrich_job) {
                    jobs::cancel_job(&conn, &enrich_job);
                    jobs::emit_progress(&app, &conn, &enrich_job, "enrich_media", "cancelled", i as i64, enrich_total, None);
                    break;
                }
                crate::commands::ai::try_embed_image(&ai, &conn, media_id, path);
                crate::commands::ai::try_extract_ocr_text(&ai, &conn, media_id, path);
                jobs::emit_progress(
                    &app,
                    &conn,
                    &enrich_job,
                    "enrich_media",
                    "running",
                    i as i64 + 1,
                    enrich_total,
                    None,
                );
            }
            jobs::emit_progress(&app, &conn, &enrich_job, "enrich_media", "completed", enrich_total, enrich_total, None);
        }
    });

    Ok(job_id_ret)
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    state.cancelled_jobs.lock().unwrap().insert(job_id.clone());
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    jobs::cancel_job(&conn, &job_id);
    Ok(())
}

#[tauri::command]
pub fn get_media_page(
    state: State<'_, AppState>,
    limit: i64,
    offset: i64,
    media_type: Option<String>,
    favorites_only: Option<bool>,
    folder_id: Option<String>,
) -> Result<MediaPage, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut where_clauses = vec!["is_trashed = 0".to_string()];
    if let Some(mt) = &media_type {
        where_clauses.push(format!("media_type = '{}'", mt.replace('\'', "")));
    }
    if favorites_only.unwrap_or(false) {
        where_clauses.push("is_favorite = 1".to_string());
    }
    if let Some(fid) = &folder_id {
        where_clauses.push(format!("folder_id = '{}'", fid.replace('\'', "")));
    }
    let where_sql = where_clauses.join(" AND ");

    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM media_items WHERE {where_sql}"),
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT id FROM media_items WHERE {where_sql}
             ORDER BY COALESCE(taken_at, created_at) DESC LIMIT ?1 OFFSET ?2"
        ))
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![limit, offset], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let items = ids
        .iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(MediaPage { items, total })
}

#[tauri::command]
pub fn search_media(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    let fts_query = format!(
        "{}*",
        trimmed
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("* ")
            .replace('"', "")
    );

    let mut stmt = conn
        .prepare(
            "SELECT m.id FROM media_fts f
             JOIN media_items m ON m.id = f.media_id
             WHERE media_fts MATCH ?1 AND m.is_trashed = 0
             ORDER BY rank LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![fts_query, limit], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_media_detail(state: State<'_, AppState>, media_id: String) -> Result<MediaItem, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    row_to_media_item(&conn, &media_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, media_id: String, favorite: bool) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE media_items SET is_favorite = ?1 WHERE id = ?2",
        params![favorite as i64, media_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns raw bytes via `tauri::ipc::Response` rather than `Vec<u8>` — a `Vec<u8>` return
/// gets JSON-encoded as an array of numbers (each byte costs ~3-4 text characters plus
/// serde overhead), which made thumbnail loading dramatically slower than just shipping the
/// bytes directly over IPC. The frontend reads this as an ArrayBuffer.
#[tauri::command]
pub fn read_media_bytes(
    state: State<'_, AppState>,
    media_id: String,
    variant: String,
) -> Result<tauri::ipc::Response, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let path: String = if variant == "original" {
        conn.query_row(
            "SELECT path FROM media_items WHERE id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    } else {
        conn.query_row(
            "SELECT path FROM thumbnails WHERE media_id = ?1 AND size = ?2",
            params![media_id, variant],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?
    };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub fn set_trashed(state: State<'_, AppState>, media_id: String, trashed: bool) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE media_items SET is_trashed = ?1 WHERE id = ?2",
        params![trashed as i64, media_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_trash(state: State<'_, AppState>, limit: i64, offset: i64) -> Result<MediaPage, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM media_items WHERE is_trashed = 1", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id FROM media_items WHERE is_trashed = 1
             ORDER BY modified_at DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![limit, offset], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let items = ids
        .iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(MediaPage { items, total })
}

#[tauri::command]
pub fn delete_media_permanently(state: State<'_, AppState>, media_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let path: String = conn
        .query_row("SELECT path FROM media_items WHERE id = ?1", params![media_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&path);

    let mut stmt = conn
        .prepare("SELECT path FROM thumbnails WHERE media_id = ?1")
        .map_err(|e| e.to_string())?;
    let thumb_paths: Vec<String> = stmt
        .query_map(params![media_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for thumb_path in thumb_paths {
        let _ = std::fs::remove_file(thumb_path);
    }

    conn.execute("DELETE FROM media_items WHERE id = ?1", params![media_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM media_fts WHERE media_id = ?1", params![media_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_places(state: State<'_, AppState>) -> Result<Vec<PlaceCluster>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT ROUND(e.gps_lat, 1) AS lat, ROUND(e.gps_lon, 1) AS lon,
                    COUNT(*) AS count, MIN(m.id) AS cover_media_id
             FROM exif_data e
             JOIN media_items m ON m.id = e.media_id
             WHERE e.gps_lat IS NOT NULL AND e.gps_lon IS NOT NULL AND m.is_trashed = 0
             GROUP BY lat, lon
             ORDER BY count DESC",
        )
        .map_err(|e| e.to_string())?;

    let clusters = stmt
        .query_map([], |r| {
            Ok(PlaceCluster {
                lat: r.get(0)?,
                lon: r.get(1)?,
                count: r.get(2)?,
                cover_media_id: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(clusters)
}

#[tauri::command]
pub fn get_library_stats(state: State<'_, AppState>) -> Result<LibraryStats, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM media_items WHERE is_trashed = 0),
            (SELECT COALESCE(SUM(size), 0) FROM media_items WHERE is_trashed = 0),
            (SELECT COUNT(*) FROM media_items WHERE is_favorite = 1 AND is_trashed = 0),
            (SELECT COUNT(*) FROM media_items WHERE is_trashed = 1)",
        [],
        |r| {
            Ok(LibraryStats {
                total_items: r.get(0)?,
                total_bytes: r.get(1)?,
                favorites: r.get(2)?,
                trashed: r.get(3)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}
