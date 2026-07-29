//! Library maintenance that is not duplicate detection — that lives in
//! `commands::duplicates`, which compares images perceptually rather than
//! byte-for-byte.

use crate::commands::media::row_to_media_item;
use crate::models::{ExportReport, FolderUsage, LibraryHealth, StorageStats};
use crate::state::AppState;
use crate::thumbnails;
use rusqlite::params;
use std::path::{Path, PathBuf};
use tauri::State;

/// Walks every indexed row and checks the file behind it. "Missing" means the
/// path is gone; "broken" means the file is there but no longer decodes.
/// Videos are only checked for existence — decoding them is the AI pipeline's job.
#[tauri::command]
pub fn scan_library_health(state: State<'_, AppState>) -> Result<LibraryHealth, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let rows: Vec<(String, String, String)> = conn
        .prepare("SELECT id, path, media_type FROM media_items WHERE is_trashed = 0")
        .map_err(|e| e.to_string())?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    let checked = rows.len() as i64;
    let mut missing_ids = Vec::new();
    let mut broken_ids = Vec::new();

    for (id, path, media_type) in rows {
        if !Path::new(&path).exists() {
            missing_ids.push(id);
        } else if media_type == "image" && image::image_dimensions(&path).is_err() {
            broken_ids.push(id);
        }
    }

    let collect = |ids: Vec<String>| {
        ids.iter()
            .map(|id| row_to_media_item(&conn, id))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    };

    Ok(LibraryHealth {
        checked,
        missing: collect(missing_ids)?,
        broken: collect(broken_ids)?,
    })
}

/// Drops rows whose file no longer exists. Nothing is deleted on disk — there is
/// nothing left to delete.
#[tauri::command]
pub fn remove_missing_entries(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let rows: Vec<(String, String)> = conn
        .prepare("SELECT id, path FROM media_items")
        .map_err(|e| e.to_string())?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    let mut removed = 0i64;
    for (id, path) in rows {
        if Path::new(&path).exists() {
            continue;
        }
        conn.execute("DELETE FROM media_fts WHERE media_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM media_items WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        removed += 1;
    }

    Ok(removed)
}

fn directory_size(dir: &Path) -> i64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|meta| meta.len() as i64)
        .sum()
}

#[tauri::command]
pub fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let count = |sql: &str| -> Result<i64, String> {
        conn.query_row(sql, [], |r| r.get(0)).map_err(|e| e.to_string())
    };

    let by_folder = conn
        .prepare(
            "SELECT f.id, f.name, f.path,
                    COUNT(m.id), COALESCE(SUM(m.size), 0)
             FROM folders f
             LEFT JOIN media_items m ON m.folder_id = f.id AND m.is_trashed = 0
             GROUP BY f.id
             ORDER BY SUM(m.size) DESC",
        )
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(FolderUsage {
                folder_id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
                item_count: r.get(3)?,
                bytes: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(StorageStats {
        total_items: count("SELECT COUNT(*) FROM media_items WHERE is_trashed = 0")?,
        image_count: count(
            "SELECT COUNT(*) FROM media_items WHERE is_trashed = 0 AND media_type = 'image'",
        )?,
        video_count: count(
            "SELECT COUNT(*) FROM media_items WHERE is_trashed = 0 AND media_type = 'video'",
        )?,
        original_bytes: count("SELECT COALESCE(SUM(size), 0) FROM media_items WHERE is_trashed = 0")?,
        thumbnail_bytes: directory_size(&thumbnails::thumbnail_dir(&state.app_data_dir)),
        database_bytes: std::fs::metadata(&state.db_path)
            .map(|meta| meta.len() as i64)
            .unwrap_or(0),
        by_folder,
    })
}

/// Deletes every generated thumbnail. They are rebuilt on the next scan, so this
/// is always safe — it only costs time, never data.
#[tauri::command]
pub fn clear_thumbnail_cache(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let dir = thumbnails::thumbnail_dir(&state.app_data_dir);
    let freed = directory_size(&dir);

    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    conn.execute("DELETE FROM thumbnails", [])
        .map_err(|e| e.to_string())?;

    Ok(freed)
}

/// Picks a filename that does not collide inside `destination`, by appending
/// " (2)", " (3)" and so on before the extension.
pub fn unique_destination(destination: &Path, filename: &str) -> PathBuf {
    let candidate = destination.join(filename);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(filename);
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for suffix in 2..10_000 {
        let candidate = destination.join(format!("{stem} ({suffix}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    candidate
}

/// Copies the chosen originals into a folder the user picked. Copies, never
/// moves: an export must not be able to damage the library.
#[tauri::command]
pub fn export_media(
    state: State<'_, AppState>,
    media_ids: Vec<String>,
    destination: String,
) -> Result<ExportReport, String> {
    let destination_dir = PathBuf::from(&destination);
    if !destination_dir.is_dir() {
        return Err("The export destination is not a folder".into());
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut exported = 0i64;
    let mut skipped = 0i64;

    for media_id in media_ids {
        let row: Result<(String, String), _> = conn.query_row(
            "SELECT path, filename FROM media_items WHERE id = ?1",
            params![media_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
        let Ok((source, filename)) = row else {
            skipped += 1;
            continue;
        };

        if !Path::new(&source).exists() {
            skipped += 1;
            continue;
        }
        match std::fs::copy(&source, unique_destination(&destination_dir, &filename)) {
            Ok(_) => exported += 1,
            Err(_) => skipped += 1,
        }
    }

    Ok(ExportReport {
        exported,
        skipped,
        destination,
    })
}
