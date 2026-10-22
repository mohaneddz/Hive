use crate::models::BackupInfo;
use crate::state::AppState;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tauri::State;

/// A restore staged here is swapped in by `db::take_pending_restore` on the next
/// start. Overwriting the database while the app holds it open is not possible on
/// Windows, so the swap has to happen before anything opens it.
pub const PENDING_RESTORE_FILE: &str = "hive.db.pending-restore";

fn create_snapshot(conn: &Connection, target: &Path) -> Result<i64, String> {
    if let Err(error) = conn.execute("VACUUM INTO ?1", rusqlite::params![target.to_string_lossy()]) {
        let _ = std::fs::remove_file(target);
        return Err(error.to_string());
    }
    let snapshot = Connection::open(target).map_err(|e| e.to_string())?;
    let integrity: String = snapshot.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if integrity != "ok" {
        let _ = std::fs::remove_file(target);
        return Err(format!("Backup integrity check failed: {integrity}"));
    }
    snapshot.query_row("SELECT COUNT(*) FROM media_items", [], |row| row.get(0)).map_err(|e| e.to_string())
}

/// Everything Hive knows lives in this one file: watched folders, albums,
/// favorites, EXIF, the search index. Thumbnails are deliberately left out — they
/// are regenerated from your photos, so backing them up would only bloat the file.
#[tauri::command]
pub fn backup_library(state: State<'_, AppState>, destination: String) -> Result<BackupInfo, String> {
    let destination_dir = PathBuf::from(&destination);
    if !destination_dir.is_dir() {
        return Err("The destination is not a folder".into());
    }

    let stamp = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let target = destination_dir.join(format!("hive-backup_{stamp}_{}.db", uuid::Uuid::new_v4().simple()));
    let item_count: i64 = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        // SQLite creates a consistent snapshot while other connections keep writing.
        // Copying the live .db file could omit pages still in the WAL.
        create_snapshot(&conn, &target)?
    };

    Ok(BackupInfo {
        bytes: std::fs::metadata(&target).map(|m| m.len() as i64).unwrap_or(0),
        path: target.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        item_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_includes_uncheckpointed_wal_data() {
        let base = std::env::temp_dir().join(format!("hive_backup_fixture_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let db_path = base.join("live.db");
        let backup_path = base.join("backup.db");
        let conn = crate::db::open(&db_path).unwrap();
        conn.execute("INSERT INTO folders (id, path, name, added_at) VALUES ('folder', 'fixture', 'Fixture', '2026-01-01')", []).unwrap();
        conn.execute("INSERT INTO media_items (id, folder_id, path, filename, hash, size, mime_type, media_type, created_at, modified_at, indexed_at)
                      VALUES ('image', 'folder', 'fixture/image.jpg', 'image.jpg', 'hash', 1, 'image/jpeg', 'image', '2026-01-01', '2026-01-01', '2026-01-01')", []).unwrap();
        assert_eq!(create_snapshot(&conn, &backup_path).unwrap(), 1);
        let backup = Connection::open(&backup_path).unwrap();
        let count: i64 = backup.query_row("SELECT COUNT(*) FROM media_items", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
        drop(backup);
        drop(conn);
        let _ = std::fs::remove_dir_all(base);
    }
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
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| "This file is not a valid Hive backup".to_string())?;
    if integrity != "ok" { return Err("This Hive backup is damaged".into()); }
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
