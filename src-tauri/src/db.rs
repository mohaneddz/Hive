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

CREATE TABLE IF NOT EXISTS people (
    id TEXT PRIMARY KEY,
    name TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS faces (
    id TEXT PRIMARY KEY,
    media_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    person_id TEXT REFERENCES people(id) ON DELETE SET NULL,
    x0 INTEGER NOT NULL,
    y0 INTEGER NOT NULL,
    x1 INTEGER NOT NULL,
    y1 INTEGER NOT NULL,
    embedding BLOB NOT NULL,
    crop_path TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS albums (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    cover_media_id TEXT REFERENCES media_items(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS album_items (
    album_id TEXT NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    media_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY (album_id, media_id)
);

CREATE INDEX IF NOT EXISTS idx_album_items_media ON album_items(media_id);

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Place names looked up once and kept, so the map can be labelled offline ever after.
CREATE TABLE IF NOT EXISTS geocode_cache (
    lat REAL NOT NULL,
    lon REAL NOT NULL,
    name TEXT NOT NULL,
    looked_up_at TEXT NOT NULL,
    PRIMARY KEY (lat, lon)
);
CREATE INDEX IF NOT EXISTS idx_media_items_trashed ON media_items(is_trashed);
CREATE INDEX IF NOT EXISTS idx_faces_media ON faces(media_id);
CREATE INDEX IF NOT EXISTS idx_faces_person ON faces(person_id);
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
    ensure_column(&conn, "media_items", "face_scanned", "INTEGER NOT NULL DEFAULT 0")?;
    // Trash, filing and editor fields. `taken_at_override` is deliberately separate
    // from `taken_at`: a rescan rewrites the latter from EXIF, which would silently
    // undo a date the user corrected by hand.
    ensure_column(&conn, "media_items", "trashed_at", "TEXT")?;
    ensure_column(&conn, "media_items", "is_hidden", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(&conn, "media_items", "is_archived", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(&conn, "media_items", "last_viewed_at", "TEXT")?;
    ensure_column(&conn, "media_items", "title", "TEXT")?;
    ensure_column(&conn, "media_items", "description", "TEXT")?;
    ensure_column(&conn, "media_items", "taken_at_override", "TEXT")?;
    ensure_column(&conn, "media_items", "edited_at", "TEXT")?;
    // Sharpness, measured once and kept so a rescan is instant.
    ensure_column(&conn, "media_items", "blur_score", "REAL")?;

    // Indexes over the columns just added. They cannot sit in MIGRATIONS: on a
    // database created by an earlier version those columns do not exist yet when
    // that batch runs, and the whole open() fails.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_media_items_viewed ON media_items(last_viewed_at);
         CREATE INDEX IF NOT EXISTS idx_media_items_hidden ON media_items(is_hidden);
         CREATE INDEX IF NOT EXISTS idx_media_items_archived ON media_items(is_archived);",
    )?;

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
