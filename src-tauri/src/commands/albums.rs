use crate::models::Album;
use crate::state::AppState;
use rusqlite::params;
use tauri::State;

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Albums are ordered by most recently touched, so the one you are working on
/// stays at the front.
#[tauri::command]
pub fn list_albums(state: State<'_, AppState>) -> Result<Vec<Album>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.name, a.description, a.cover_media_id,
                    (SELECT COUNT(*) FROM album_items i WHERE i.album_id = a.id),
                    a.created_at, a.updated_at
             FROM albums a
             ORDER BY a.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Album {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                cover_media_id: r.get(3)?,
                item_count: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_album(state: State<'_, AppState>, album_id: String) -> Result<Album, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT a.id, a.name, a.description, a.cover_media_id,
                (SELECT COUNT(*) FROM album_items i WHERE i.album_id = a.id),
                a.created_at, a.updated_at
         FROM albums a WHERE a.id = ?1",
        params![album_id],
        |r| {
            Ok(Album {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                cover_media_id: r.get(3)?,
                item_count: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_album(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
) -> Result<Album, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("An album needs a name".into());
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = now();
    conn.execute(
        "INSERT INTO albums (id, name, description, cover_media_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?4)",
        params![id, name, description, timestamp],
    )
    .map_err(|e| e.to_string())?;

    Ok(Album {
        id,
        name,
        description,
        cover_media_id: None,
        item_count: 0,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

#[tauri::command]
pub fn update_album(
    state: State<'_, AppState>,
    album_id: String,
    name: String,
    description: Option<String>,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("An album needs a name".into());
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE albums SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
        params![name, description, now(), album_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Deleting an album never touches the photos: `album_items` cascades away, the
/// media rows stay exactly where they were.
#[tauri::command]
pub fn delete_album(state: State<'_, AppState>, album_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM albums WHERE id = ?1", params![album_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_album_cover(
    state: State<'_, AppState>,
    album_id: String,
    media_id: Option<String>,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE albums SET cover_media_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![media_id, now(), album_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Adds items to an album, ignoring the ones already in it. The first item added
/// becomes the cover so an album is never a blank tile.
#[tauri::command]
pub fn add_media_to_album(
    state: State<'_, AppState>,
    album_id: String,
    media_ids: Vec<String>,
) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = now();

    let mut added = 0i64;
    for media_id in &media_ids {
        let changed = conn
            .execute(
                "INSERT INTO album_items (album_id, media_id, added_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(album_id, media_id) DO NOTHING",
                params![album_id, media_id, timestamp],
            )
            .map_err(|e| e.to_string())?;
        added += changed as i64;
    }

    conn.execute(
        "UPDATE albums SET updated_at = ?1,
             cover_media_id = COALESCE(cover_media_id,
                 (SELECT media_id FROM album_items WHERE album_id = ?2 ORDER BY added_at LIMIT 1))
         WHERE id = ?2",
        params![timestamp, album_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(added)
}

#[tauri::command]
pub fn remove_media_from_album(
    state: State<'_, AppState>,
    album_id: String,
    media_id: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM album_items WHERE album_id = ?1 AND media_id = ?2",
        params![album_id, media_id],
    )
    .map_err(|e| e.to_string())?;

    // A cover pointing at a removed item would render as a broken tile.
    conn.execute(
        "UPDATE albums SET updated_at = ?1,
             cover_media_id = CASE WHEN cover_media_id = ?3
                 THEN (SELECT media_id FROM album_items WHERE album_id = ?2 ORDER BY added_at LIMIT 1)
                 ELSE cover_media_id END
         WHERE id = ?2",
        params![now(), album_id, media_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Which albums a given photo already belongs to — drives the checkmarks in the
/// "Add to album" menu.
#[tauri::command]
pub fn list_albums_for_media(
    state: State<'_, AppState>,
    media_id: String,
) -> Result<Vec<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT album_id FROM album_items WHERE media_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![media_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
