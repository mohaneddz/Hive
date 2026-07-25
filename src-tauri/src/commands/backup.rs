use crate::models::BackupInfo;
use crate::state::AppState;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tauri::State;

/// A restore staged here is swapped in by `db::take_pending_restore` on the next
/// start. Overwriting the database while the app holds it open is not possible on
/// Windows, so the swap has to happen before anything opens it.
pub const PENDING_RESTORE_FILE: &str = "hive.db.pending-restore";

/// Everything Hive knows lives in this one file: watched folders, albums,
/// favorites, EXIF, the search index. Thumbnails are deliberately left out — they
/// are regenerated from your photos, so backing them up would only bloat the file.
#[tauri::command]
pub fn backup_library(state: State<'_, AppState>, destination: String) -> Result<BackupInfo, String> {
    let destination_dir = PathBuf::from(&destination);
    if !destination_dir.is_dir() {
        return Err("The destination is not a folder".into());
    }

    let item_count: i64 = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        // Fold the write-ahead log back into the main file, otherwise a copy taken
        // now would be missing everything written since the last checkpoint.
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .map_err(|e| e.to_string())?;
        conn.query_row("SELECT COUNT(*) FROM media_items", [], |r| r.get(0))
            .map_err(|e| e.to_string())?
    };

    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let target = destination_dir.join(format!("hive-backup_{stamp}.db"));
    std::fs::copy(&state.db_path, &target).map_err(|e| e.to_string())?;

    Ok(BackupInfo {
        bytes: std::fs::metadata(&target).map(|m| m.len() as i64).unwrap_or(0),
        path: target.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        item_count,
    })
}

/// Reads a backup without committing to it, so the UI can say what is inside
/// before the user replaces anything.
#[tauri::command]
pub fn inspect_backup(backup_path: String) -> Result<BackupInfo, String> {
    let path = PathBuf::from(&backup_path);
    if !path.is_file() {
        return Err("That file does not exist".into());
    }

    let conn = Connection::open(&path).map_err(|_| "This file is not a Hive backup".to_string())?;
    let item_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM media_items", [], |r| r.get(0))
        .map_err(|_| "This file is not a Hive backup".to_string())?;

    let created_at = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339())
        .unwrap_or_default();

    Ok(BackupInfo {
        bytes: std::fs::metadata(&path).map(|m| m.len() as i64).unwrap_or(0),
        path: backup_path,
        created_at,
        item_count,
    })
}

/// Stages a backup to replace the live database. Nothing is overwritten yet — the
/// swap happens at the next start, which is the only moment the database is not
/// already open.
#[tauri::command]
pub fn restore_library(state: State<'_, AppState>, backup_path: String) -> Result<String, String> {
    // Validate before staging: a corrupt file must not be able to strand the app.
    let info = inspect_backup(backup_path.clone())?;

    let staged = state.app_data_dir.join(PENDING_RESTORE_FILE);
    std::fs::copy(&backup_path, &staged).map_err(|e| e.to_string())?;

    Ok(format!(
        "Backup staged ({} items). Close and reopen Hive to finish restoring.",
        info.item_count
    ))
}

/// Drops a staged restore that has not been applied yet.
#[tauri::command]
pub fn cancel_pending_restore(state: State<'_, AppState>) -> Result<bool, String> {
    let staged = state.app_data_dir.join(PENDING_RESTORE_FILE);
    if !staged.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&staged).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn has_pending_restore(state: State<'_, AppState>) -> bool {
    state.app_data_dir.join(PENDING_RESTORE_FILE).exists()
}

/// Applies a staged restore, if there is one. Called at startup before the
/// database is opened. Returns whether a swap happened.
pub fn take_pending_restore(app_data_dir: &Path, db_path: &Path) -> std::io::Result<bool> {
    let staged = app_data_dir.join(PENDING_RESTORE_FILE);
    if !staged.exists() {
        return Ok(false);
    }

    // The side files belong to the database being replaced; leaving them behind
    // would let SQLite replay a log that no longer matches.
    for suffix in ["-wal", "-shm"] {
        let side_file = PathBuf::from(format!("{}{suffix}", db_path.to_string_lossy()));
        let _ = std::fs::remove_file(side_file);
    }
    let _ = std::fs::remove_file(db_path);

    std::fs::rename(&staged, db_path)?;
    Ok(true)
}
