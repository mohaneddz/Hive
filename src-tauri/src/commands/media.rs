use crate::models::{ExifData, LibraryStats, MediaItem, MediaPage, PlaceCluster};
use crate::state::AppState;
use crate::{db, indexing, jobs, thumbnails};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, State};

pub(crate) fn row_to_media_item(conn: &Connection, id: &str) -> rusqlite::Result<MediaItem> {
    let mut item = conn.query_row(
        "SELECT id, folder_id, path, filename, hash, size, width, height, duration_ms,
                mime_type, media_type, taken_at, created_at, modified_at, indexed_at,
                is_favorite, is_trashed, trashed_at, is_hidden, is_archived, last_viewed_at,
                title, description, taken_at_override, edited_at
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
                trashed_at: r.get(17)?,
                is_hidden: r.get::<_, i64>(18)? != 0,
                is_archived: r.get::<_, i64>(19)? != 0,
                last_viewed_at: r.get(20)?,
                title: r.get(21)?,
                description: r.get(22)?,
                taken_at_override: r.get(23)?,
                edited_at: r.get(24)?,
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
                    let _ = thumbnails::generate(
                        &conn,
                        &app_data_dir,
                        &indexed.item.id,
                        path,
                        &indexed.item.media_type,
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

/// Which slice of the library a query is asking for. These are mutually exclusive
/// views, so one string beats three overlapping booleans that could ask for
/// "trashed but not trashed".
pub fn scope_predicate(scope: Option<&str>) -> &'static str {
    match scope.unwrap_or("library") {
        "trash" => "is_trashed = 1",
        "hidden" => "is_trashed = 0 AND is_hidden = 1",
        "archive" => "is_trashed = 0 AND is_archived = 1",
        "all" => "is_trashed = 0",
        // The default library view hides everything the user filed away.
        _ => "is_trashed = 0 AND is_hidden = 0 AND is_archived = 0",
    }
}

/// Sort orders are picked from this fixed set, never built from caller text.
fn order_predicate(sort: Option<&str>, scope: Option<&str>) -> &'static str {
    match sort {
        Some("added") => "indexed_at DESC",
        Some("name") => "filename COLLATE NOCASE ASC",
        Some("size") => "size DESC",
        Some("oldest") => "COALESCE(taken_at_override, taken_at, created_at) ASC",
        Some("viewed") => "last_viewed_at DESC",
        // The trash reads as a deletion log; everything else as a timeline.
        _ if scope == Some("trash") => "COALESCE(trashed_at, modified_at) DESC",
        // A date the user corrected by hand outranks the one read from EXIF.
        _ => "COALESCE(taken_at_override, taken_at, created_at) DESC",
    }
}

/// Filters that apply on top of the scope. Every caller-supplied value is a bound
/// parameter with a fixed placeholder count — a NULL simply switches its clause
/// off — so nothing is ever spliced into the SQL text.
const MEDIA_FILTER_SQL: &str = "(?1 IS NULL OR media_type = ?1)
     AND (?2 = 0 OR is_favorite = 1)
     AND (?3 IS NULL OR folder_id = ?3)
     AND (?4 IS NULL OR id IN (SELECT media_id FROM album_items WHERE album_id = ?4))";

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn get_media_page(
    state: State<'_, AppState>,
    limit: i64,
    offset: i64,
    media_type: Option<String>,
    favorites_only: Option<bool>,
    folder_id: Option<String>,
    album_id: Option<String>,
    scope: Option<String>,
    sort: Option<String>,
) -> Result<MediaPage, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let favorites = favorites_only.unwrap_or(false) as i64;
    let scope_sql = scope_predicate(scope.as_deref());
    let order_sql = order_predicate(sort.as_deref(), scope.as_deref());

    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM media_items WHERE {scope_sql} AND {MEDIA_FILTER_SQL}"),
            params![media_type, favorites, folder_id, album_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT id FROM media_items WHERE {scope_sql} AND {MEDIA_FILTER_SQL}
             ORDER BY {order_sql} LIMIT ?5 OFFSET ?6"
        ))
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(
            params![media_type, favorites, folder_id, album_id, limit, offset],
            |r| r.get(0),
        )
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
pub fn set_favorite(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    favorite: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE media_items SET is_favorite = ?1 WHERE id = ?2",
        params![favorite as i64, media_id],
    )
    .map_err(|e| e.to_string())?;
    let _ = tauri::Emitter::emit(&app, EVENT_MEDIA_FLAGGED, &media_id);
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

/// Emitted when a flag changes but the set of files did not.
///
/// Deliberately separate from `media:changed`: grids react to that one by
/// refetching a whole page, which would make every heart click flicker and reset
/// scrolling. Counters listen to this instead and only re-read their totals.
pub const EVENT_MEDIA_FLAGGED: &str = "media:flagged";

/// Moves an item to the trash (`trashed = true`) or restores it. Nothing is
/// touched on disk here — only the flag and its timestamp.
#[tauri::command]
pub fn set_trashed(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    trashed: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let trashed_at = trashed.then(|| chrono::Utc::now().to_rfc3339());
    conn.execute(
        "UPDATE media_items SET is_trashed = ?1, trashed_at = ?2 WHERE id = ?3",
        params![trashed as i64, trashed_at, media_id],
    )
    .map_err(|e| e.to_string())?;
    let _ = tauri::Emitter::emit(&app, "media:changed", &media_id);
    Ok(())
}

/// Hidden items drop out of the library, search and every album view; they are
/// only reachable from Collections › Hidden.
#[tauri::command]
pub fn set_hidden(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    hidden: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE media_items SET is_hidden = ?1 WHERE id = ?2",
        params![hidden as i64, media_id],
    )
    .map_err(|e| e.to_string())?;
    let _ = tauri::Emitter::emit(&app, EVENT_MEDIA_FLAGGED, &media_id);
    Ok(())
}

/// Archived items stay searchable but leave the main timeline.
#[tauri::command]
pub fn set_archived(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    archived: bool,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE media_items SET is_archived = ?1 WHERE id = ?2",
        params![archived as i64, media_id],
    )
    .map_err(|e| e.to_string())?;
    let _ = tauri::Emitter::emit(&app, EVENT_MEDIA_FLAGGED, &media_id);
    Ok(())
}

/// Records that an item was opened, which is what "Continue where you left off"
/// on the home page reads back.
#[tauri::command]
pub fn touch_last_viewed(state: State<'_, AppState>, media_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE media_items SET last_viewed_at = ?1 WHERE id = ?2",
        params![chrono::Utc::now().to_rfc3339(), media_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Drops one item from the library for good: the original goes to the operating
/// system's recycle bin (so a mistake is still recoverable outside Hive), the
/// generated thumbnails are deleted outright, and every database row goes with it.
fn purge_media(conn: &Connection, media_id: &str) -> Result<(), String> {
    let source_path: String = conn
        .query_row(
            "SELECT path FROM media_items WHERE id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Read these before the row is gone: thumbnails cascade-delete with the item.
    let thumbnail_paths: Vec<String> = conn
        .prepare("SELECT path FROM thumbnails WHERE media_id = ?1")
        .map_err(|e| e.to_string())?
        .query_map(params![media_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|path| path.ok())
        .collect();

    if std::path::Path::new(&source_path).exists() {
        trash::delete(&source_path).map_err(|e| e.to_string())?;
    }
    for path in thumbnail_paths {
        let _ = std::fs::remove_file(path);
    }

    // media_fts is a virtual table, so no foreign key cleans it up for us.
    conn.execute("DELETE FROM media_fts WHERE media_id = ?1", params![media_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM media_items WHERE id = ?1", params![media_id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_media_permanently(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
) -> Result<(), String> {
    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        purge_media(&conn, &media_id)?;
    }
    let _ = tauri::Emitter::emit(&app, "media:changed", &media_id);
    Ok(())
}

/// Purges every trashed item. Returns how many were removed.
#[tauri::command]
pub fn empty_trash(app: AppHandle, state: State<'_, AppState>) -> Result<i64, String> {
    let removed = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        let ids: Vec<String> = conn
            .prepare("SELECT id FROM media_items WHERE is_trashed = 1")
            .map_err(|e| e.to_string())?
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|id| id.ok())
            .collect();

        // One unreadable file must not strand the rest of the trash.
        let mut removed = 0i64;
        for id in ids {
            if purge_media(&conn, &id).is_ok() {
                removed += 1;
            }
        }
        removed
    };

    let _ = tauri::Emitter::emit(&app, "media:changed", "trash");
    Ok(removed)
}

/// "Memories": photos shot on this calendar day in an earlier year. Pure date
/// arithmetic on `taken_at` — no model, no tagging, nothing to train.
#[tauri::command]
pub fn get_on_this_day(state: State<'_, AppState>, limit: i64) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Local::now();
    let month_day = now.format("%m-%d").to_string();
    let this_year = now.format("%Y").to_string();

    let live = scope_predicate(None);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id FROM media_items
             WHERE {live} AND taken_at IS NOT NULL
               AND strftime('%m-%d', taken_at) = ?1
               AND strftime('%Y', taken_at) < ?2
             ORDER BY taken_at DESC LIMIT ?3"
        ))
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![month_day, this_year, limit], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|id| id.ok())
        .collect();

    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
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
    // `live` is the default library scope, so the headline counts match what the
    // gallery actually shows — hidden and archived items are not in it.
    let live = scope_predicate(None);
    conn.query_row(
        &format!(
            "SELECT
                (SELECT COUNT(*) FROM media_items WHERE {live}),
                (SELECT COALESCE(SUM(size), 0) FROM media_items WHERE {live}),
                (SELECT COUNT(*) FROM media_items WHERE {live} AND is_favorite = 1),
                (SELECT COUNT(*) FROM media_items WHERE is_trashed = 1),
                (SELECT COUNT(*) FROM media_items WHERE {live} AND media_type = 'image'),
                (SELECT COUNT(*) FROM media_items WHERE {live} AND media_type = 'video'),
                (SELECT COUNT(*) FROM albums),
                (SELECT COUNT(*) FROM exif_data e JOIN media_items m ON m.id = e.media_id
                 WHERE m.is_trashed = 0 AND e.gps_lat IS NOT NULL AND e.gps_lon IS NOT NULL),
                (SELECT COUNT(*) FROM media_items WHERE is_trashed = 0 AND is_hidden = 1),
                (SELECT COUNT(*) FROM media_items WHERE is_trashed = 0 AND is_archived = 1),
                (SELECT COUNT(*) FROM folders)"
        ),
        [],
        |r| {
            Ok(LibraryStats {
                total_items: r.get(0)?,
                total_bytes: r.get(1)?,
                favorites: r.get(2)?,
                trashed: r.get(3)?,
                image_count: r.get(4)?,
                video_count: r.get(5)?,
                album_count: r.get(6)?,
                place_count: r.get(7)?,
                hidden_count: r.get(8)?,
                archived_count: r.get(9)?,
                folder_count: r.get(10)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

/// Regenerates thumbnails for any already-indexed item missing one — covers items indexed
/// before video-thumbnail support existed, or where generation failed the first time (e.g.
/// ffmpeg wasn't installed yet).
#[tauri::command]
pub async fn backfill_thumbnails(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db_path = state.db_path.clone();
    let app_data_dir = state.app_data_dir.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "thumbnail_backfill").map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT m.id, m.path, m.media_type FROM media_items m
                 LEFT JOIN thumbnails t ON t.media_id = m.id AND t.size = 'sm'
                 WHERE m.is_trashed = 0 AND t.media_id IS NULL",
            )
            .map_err(|e| e.to_string())?;
        let pending: Vec<(String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let total = pending.len() as i64;
        for (i, (media_id, path, media_type)) in pending.iter().enumerate() {
            if cancelled_jobs.lock().unwrap().remove(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(&app, &conn, &job_id, "thumbnail_backfill", "cancelled", i as i64, total, None);
                return Ok(());
            }
            let _ = thumbnails::generate(&conn, &app_data_dir, media_id, std::path::Path::new(path), media_type);
            jobs::emit_progress(&app, &conn, &job_id, "thumbnail_backfill", "running", i as i64 + 1, total, None);
        }

        jobs::emit_progress(&app, &conn, &job_id, "thumbnail_backfill", "completed", total, total, None);
        let _ = tauri::Emitter::emit(&app, "media:changed", "");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
