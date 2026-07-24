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
"#;

pub fn open(db_path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(MIGRATIONS)?;
    Ok(conn)
}
