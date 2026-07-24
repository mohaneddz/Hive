use crate::models::Folder;
use crate::state::AppState;
use rusqlite::params;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, path, name, is_watched, added_at FROM folders ORDER BY added_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Folder {
                id: r.get(0)?,
                path: r.get(1)?,
                name: r.get(2)?,
                is_watched: r.get::<_, i64>(3)? != 0,
                added_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_watched_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Folder, String> {
    let folder_path = std::path::Path::new(&path);
    if !folder_path.is_dir() {
        return Err("Selected path is not a directory".into());
    }
    let name = folder_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO folders (id, path, name, is_watched, added_at) VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT(path) DO NOTHING",
            params![id, path, name, now],
        )
        .map_err(|e| e.to_string())?;
    }

    let folder_id: String = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row("SELECT id FROM folders WHERE path = ?1", params![path], |r| r.get(0))
            .map_err(|e| e.to_string())?
    };

    state
        .watchers
        .watch(
            app,
            state.db_path.clone(),
            state.app_data_dir.clone(),
            folder_id.clone(),
            folder_path.to_path_buf(),
        )
        .map_err(|e| e.to_string())?;

    Ok(Folder {
        id: folder_id,
        path,
        name,
        is_watched: true,
        added_at: now,
    })
}

#[tauri::command]
pub fn remove_watched_folder(state: State<'_, AppState>, folder_id: String) -> Result<(), String> {
    state.watchers.unwatch(&folder_id);
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![folder_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
