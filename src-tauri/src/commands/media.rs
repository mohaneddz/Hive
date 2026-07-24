use crate::models::{ExifData, MediaItem, MediaPage, ScanResult};
use crate::state::AppState;
use crate::{db, indexing, jobs, thumbnails};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, State};

fn row_to_media_item(conn: &Connection, id: &str) -> rusqlite::Result<MediaItem> {
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
pub async fn scan_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: String,
) -> Result<ScanResult, String> {
    let db_path = state.db_path.clone();
    let app_data_dir = state.app_data_dir.clone();

    let folder_path: String = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT path FROM folders WHERE id = ?1", params![folder_id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?
    };

    tauri::async_runtime::spawn_blocking(move || -> Result<ScanResult, String> {
        let conn = db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "scan_folder").map_err(|e| e.to_string())?;

        let files = indexing::walk_folder(std::path::Path::new(&folder_path));
        let total = files.len() as i64;
        let mut added = 0i64;
        let mut skipped = 0i64;

        for (i, path) in files.iter().enumerate() {
            match indexing::index_file(&conn, &folder_id, path) {
                Ok(Some(indexed)) => {
                    added += 1;
                    let _ = thumbnails::generate_for_image(
                        &conn,
                        &app_data_dir,
                        &indexed.item.id,
                        path,
                    );
                }
                Ok(None) => skipped += 1,
                Err(e) => {
                    skipped += 1;
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

        Ok(ScanResult {
            folder_id,
            scanned: total,
            added,
            skipped,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_media_page(
    state: State<'_, AppState>,
    limit: i64,
    offset: i64,
    media_type: Option<String>,
    favorites_only: Option<bool>,
) -> Result<MediaPage, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut where_clauses = vec!["is_trashed = 0".to_string()];
    if let Some(mt) = &media_type {
        where_clauses.push(format!("media_type = '{}'", mt.replace('\'', "")));
    }
    if favorites_only.unwrap_or(false) {
        where_clauses.push("is_favorite = 1".to_string());
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

#[tauri::command]
pub fn read_media_bytes(
    state: State<'_, AppState>,
    media_id: String,
    variant: String,
) -> Result<Vec<u8>, String> {
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
    std::fs::read(&path).map_err(|e| e.to_string())
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
