//! Preferences that outlive a session: how much disk the thumbnail cache may
//! take, and which key does what.
//!
//! Both live in the `app_settings` table rather than the frontend's local
//! storage, because both are read by Rust as well as by the interface.

use crate::models::{CacheReport, NsfwPolicy};
use crate::state::AppState;
use crate::thumbnails;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use tauri::State;

const SETTING_CACHE_LIMIT: &str = "thumbnail_cache_limit_mb";
const SETTING_SHORTCUTS: &str = "shortcuts";
const SETTING_NSFW_THRESHOLD: &str = "nsfw_threshold";
const SETTING_NSFW_AUTO_HIDE: &str = "nsfw_auto_hide";

/// 0 means "no limit". Anything else is a ceiling in megabytes.
const DEFAULT_CACHE_LIMIT_MB: i64 = 0;

/// The score the classifier has to reach before a photo counts as sensitive.
const DEFAULT_NSFW_THRESHOLD: f64 = 0.7;

fn read_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

fn write_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/* ---------------------------------------------------- sensitive content -- */

/// Reads the policy, falling back to the defaults when nothing was ever set.
///
/// Auto-hide defaults to **off**. Hiding is not a display choice — it takes the
/// photo out of the library — and no classifier is right often enough to do that
/// to someone's pictures without being asked. Covering them in the grid, which is
/// reversible with one click, happens either way.
pub fn nsfw_policy(conn: &Connection) -> NsfwPolicy {
    NsfwPolicy {
        threshold: read_setting(conn, SETTING_NSFW_THRESHOLD)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| (0.0..=1.0).contains(value))
            .unwrap_or(DEFAULT_NSFW_THRESHOLD),
        auto_hide: read_setting(conn, SETTING_NSFW_AUTO_HIDE).as_deref() == Some("true"),
    }
}

#[tauri::command]
pub fn get_nsfw_policy(state: State<'_, AppState>) -> Result<NsfwPolicy, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    Ok(nsfw_policy(&conn))
}

#[tauri::command]
pub fn set_nsfw_policy(
    state: State<'_, AppState>,
    threshold: f64,
    auto_hide: bool,
) -> Result<(), String> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err("Threshold must be between 0 and 1".into());
    }
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    write_setting(&conn, SETTING_NSFW_THRESHOLD, &threshold.to_string())?;
    write_setting(&conn, SETTING_NSFW_AUTO_HIDE, if auto_hide { "true" } else { "false" })
}

/* ------------------------------------------------------ thumbnail cache -- */

#[tauri::command]
pub fn get_cache_limit_mb(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    Ok(read_setting(&conn, SETTING_CACHE_LIMIT)
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_CACHE_LIMIT_MB))
}

#[tauri::command]
pub fn set_cache_limit_mb(state: State<'_, AppState>, megabytes: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    write_setting(&conn, SETTING_CACHE_LIMIT, &megabytes.max(0).to_string())
}

/// Deletes the least recently used thumbnails until the cache fits its limit.
///
/// "Least recently used" comes from the file's own access-or-modified time, so a
/// thumbnail you keep looking at survives even if the photo behind it is old.
/// Nothing is lost by this: thumbnails are regenerated on demand from your photos.
///
/// Safe to call at any time — with no limit set it does nothing at all.
pub fn enforce_cache_limit(conn: &Connection, app_data_dir: &std::path::Path) -> CacheReport {
    let limit_mb = read_setting(conn, SETTING_CACHE_LIMIT)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_CACHE_LIMIT_MB);

    let directory = thumbnails::thumbnail_dir(app_data_dir);
    let mut files: Vec<(std::path::PathBuf, i64, std::time::SystemTime)> =
        walkdir::WalkDir::new(&directory)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                let touched = metadata.accessed().or_else(|_| metadata.modified()).ok()?;
                Some((entry.path().to_path_buf(), metadata.len() as i64, touched))
            })
            .collect();

    let used: i64 = files.iter().map(|(_, size, _)| size).sum();
    let limit_bytes = limit_mb * 1024 * 1024;

    if limit_mb <= 0 || used <= limit_bytes {
        return CacheReport {
            used_bytes: used,
            limit_bytes,
            freed_bytes: 0,
            removed: 0,
        };
    }

    // Oldest first, so the ones you have not looked at in a while go first.
    files.sort_by_key(|(_, _, touched)| *touched);

    let mut freed = 0i64;
    let mut removed = 0i64;
    for (path, size, _) in files {
        if used - freed <= limit_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            // The row has to go too, or the UI would keep asking for a file that
            // is no longer there.
            let _ = conn.execute(
                "DELETE FROM thumbnails WHERE path = ?1",
                params![path.to_string_lossy()],
            );
            freed += size;
            removed += 1;
        }
    }

    CacheReport {
        used_bytes: used - freed,
        limit_bytes,
        freed_bytes: freed,
        removed,
    }
}

#[tauri::command]
pub fn apply_cache_limit(state: State<'_, AppState>) -> Result<CacheReport, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    Ok(enforce_cache_limit(&conn, &state.app_data_dir))
}

/* ------------------------------------------------------------ shortcuts -- */

/// Only the bindings that differ from the defaults are stored, so changing a
/// default later reaches everyone who never touched that key.
#[tauri::command]
pub fn get_shortcut_overrides(state: State<'_, AppState>) -> Result<HashMap<String, String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    Ok(read_setting(&conn, SETTING_SHORTCUTS)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn set_shortcut_overrides(
    state: State<'_, AppState>,
    overrides: HashMap<String, String>,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let encoded = serde_json::to_string(&overrides).map_err(|e| e.to_string())?;
    write_setting(&conn, SETTING_SHORTCUTS, &encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE thumbnails (media_id TEXT, size TEXT, path TEXT);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn hiding_sensitive_photos_is_off_until_it_is_asked_for() {
        let conn = memory_db();
        let policy = nsfw_policy(&conn);

        assert_eq!(policy.threshold, DEFAULT_NSFW_THRESHOLD);
        assert!(!policy.auto_hide, "photos must not vanish by default");
    }

    #[test]
    fn a_threshold_outside_zero_to_one_falls_back_to_the_default() {
        let conn = memory_db();
        // Nothing rejects a hand-edited database, so a nonsense value must not
        // become a threshold no photo can ever reach.
        write_setting(&conn, SETTING_NSFW_THRESHOLD, "7").unwrap();

        assert_eq!(nsfw_policy(&conn).threshold, DEFAULT_NSFW_THRESHOLD);
    }

    #[test]
    fn a_saved_policy_reads_back_as_written() {
        let conn = memory_db();
        write_setting(&conn, SETTING_NSFW_THRESHOLD, "0.85").unwrap();
        write_setting(&conn, SETTING_NSFW_AUTO_HIDE, "true").unwrap();

        let policy = nsfw_policy(&conn);
        assert_eq!(policy.threshold, 0.85);
        assert!(policy.auto_hide);
    }

    #[test]
    fn an_unset_limit_reads_as_unlimited() {
        let conn = memory_db();
        assert!(read_setting(&conn, SETTING_CACHE_LIMIT).is_none());
    }

    #[test]
    fn writing_a_setting_twice_updates_rather_than_duplicates() {
        let conn = memory_db();
        write_setting(&conn, SETTING_CACHE_LIMIT, "500").unwrap();
        write_setting(&conn, SETTING_CACHE_LIMIT, "800").unwrap();

        assert_eq!(read_setting(&conn, SETTING_CACHE_LIMIT).as_deref(), Some("800"));
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_settings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn no_limit_means_nothing_is_deleted() {
        let conn = memory_db();
        let report = enforce_cache_limit(&conn, std::path::Path::new("does-not-exist"));
        assert_eq!(report.removed, 0);
        assert_eq!(report.freed_bytes, 0);
    }

    #[test]
    fn overrides_survive_a_round_trip() {
        let conn = memory_db();
        let mut overrides = HashMap::new();
        overrides.insert("next".to_string(), "d".to_string());
        write_setting(&conn, SETTING_SHORTCUTS, &serde_json::to_string(&overrides).unwrap())
            .unwrap();

        let decoded: HashMap<String, String> =
            serde_json::from_str(&read_setting(&conn, SETTING_SHORTCUTS).unwrap()).unwrap();
        assert_eq!(decoded.get("next").map(String::as_str), Some("d"));
    }
}
