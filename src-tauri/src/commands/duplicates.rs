use crate::commands::media::row_to_media_item;
use crate::models::DuplicateGroup;
use crate::state::AppState;
use crate::{duplicates, jobs};
use rusqlite::params;
use std::collections::HashMap;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn scan_duplicates(app: AppHandle, state: State<'_, AppState>) -> Result<usize, String> {
    let db_path = state.db_path.clone();

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "scan_duplicates").map_err(|e| e.to_string())?;
    jobs::emit_progress(&app, &conn, &job_id, "scan_duplicates", "running", 0, 1, None);

    let result = tauri::async_runtime::spawn_blocking(move || {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        duplicates::recompute_duplicate_groups(&conn).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;

    match &result {
        Ok(_) => jobs::emit_progress(&app, &conn, &job_id, "scan_duplicates", "completed", 1, 1, None),
        Err(e) => jobs::fail_job(&conn, &job_id, e),
    }

    result
}

#[tauri::command]
pub fn get_duplicate_groups(state: State<'_, AppState>) -> Result<Vec<DuplicateGroup>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT group_id, media_id FROM duplicates ORDER BY group_id, similarity DESC")
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut ordered_groups: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
    for (group_id, media_id) in rows {
        if !grouped.contains_key(&group_id) {
            ordered_groups.push(group_id.clone());
        }
        grouped.entry(group_id).or_default().push(media_id);
    }

    ordered_groups
        .into_iter()
        .map(|group_id| {
            let media_ids = &grouped[&group_id];
            let items = media_ids
                .iter()
                .map(|id| row_to_media_item(&conn, id))
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            Ok(DuplicateGroup { group_id, items })
        })
        .collect::<Result<Vec<_>, String>>()
}

#[tauri::command]
pub fn dismiss_duplicate_group(state: State<'_, AppState>, group_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM duplicates WHERE group_id = ?1", params![group_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
