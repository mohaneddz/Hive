use rusqlite::Connection;
use std::path::Path;

pub const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    is_watched INTEGER NOT NULL DEFAULT 1,
    added_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS media_items (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    width INTEGER,
    height INTEGER,
    duration_ms INTEGER,
    mime_type TEXT NOT NULL,
    media_type TEXT NOT NULL,
    taken_at TEXT,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    indexed_at TEXT NOT NULL,
    is_favorite INTEGER NOT NULL DEFAULT 0,
    is_trashed INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_media_items_folder ON media_items(folder_id);
CREATE INDEX IF NOT EXISTS idx_media_items_taken_at ON media_items(taken_at);
CREATE INDEX IF NOT EXISTS idx_media_items_hash ON media_items(hash);

CREATE TABLE IF NOT EXISTS exif_data (
    media_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    camera_make TEXT,
    camera_model TEXT,
    lens TEXT,
    iso INTEGER,
    f_number REAL,
    exposure_time TEXT,
    focal_length REAL,
    gps_lat REAL,
    gps_lon REAL
);

CREATE TABLE IF NOT EXISTS thumbnails (
    media_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    size TEXT NOT NULL,
    path TEXT NOT NULL,
    PRIMARY KEY (media_id, size)
);

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    status TEXT NOT NULL,
    payload TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS duplicates (
    id TEXT PRIMARY KEY,
    group_id TEXT NOT NULL,
    media_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    similarity REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_duplicates_group ON duplicates(group_id);

CREATE VIRTUAL TABLE IF NOT EXISTS media_fts USING fts5(
    media_id UNINDEXED,
    filename,
    camera_make,
    camera_model,
    ocr_text,
    tokenize = 'porter unicode61'
);

CREATE TABLE IF NOT EXISTS embeddings (
    media_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    dim INTEGER NOT NULL,
    vector BLOB NOT NULL,
    created_at TEXT NOT NULL
);
"#;

/// Adds `column` to `table` if it isn't there yet. `CREATE TABLE IF NOT EXISTS` in MIGRATIONS
/// is idempotent for new tables, but `ALTER TABLE ADD COLUMN` isn't — this covers the "existing
/// table gains a column" case for schema evolution without a full migration-versioning system.
fn ensure_column(conn: &Connection, table: &str, column: &str, decl: &str) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let existing: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !existing.iter().any(|c| c == column) {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])?;
    }
    Ok(())
}

pub fn open(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(MIGRATIONS)?;
    ensure_column(&conn, "media_items", "phash", "TEXT")?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_open() {
        let db_path = std::env::temp_dir().join(format!("hive_test_{}.db", uuid::Uuid::new_v4()));
        let conn = open(&db_path).expect("Failed to open database");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");
        let _ = std::fs::remove_file(db_path);
    }
}
