use crate::commands::media::row_to_media_item;
use crate::models::MediaPage;
use crate::state::AppState;
use rusqlite::Connection;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    query: String,
    semantic: bool,
    media_type: Option<String>,
    favorites_only: bool,
    date_from: Option<String>,
    date_to: Option<String>,
    camera: Option<String>,
    person_id: Option<String>,
    orientation: Option<String>,
    sort: String,
    limit: i64,
    offset: i64,
}

fn filter_sql() -> &'static str {
    "m.is_trashed = 0 AND m.is_hidden = 0 AND m.is_archived = 0
     AND (?1 IS NULL OR m.media_type = ?1)
     AND (?2 = 0 OR m.is_favorite = 1)
     AND (?3 IS NULL OR COALESCE(m.taken_at_override, m.taken_at, m.created_at) >= ?3)
     AND (?4 IS NULL OR COALESCE(m.taken_at_override, m.taken_at, m.created_at) < ?4)
     AND (?5 IS NULL OR TRIM(COALESCE(e.camera_make, '') || ' ' || COALESCE(e.camera_model, '')) = ?5)
     AND (?6 IS NULL OR EXISTS (SELECT 1 FROM faces face WHERE face.media_id = m.id AND face.person_id = ?6))
     AND (?7 IS NULL OR
          (?7 = 'landscape' AND m.width > m.height) OR
          (?7 = 'portrait' AND m.height > m.width) OR
          (?7 = 'square' AND m.width = m.height AND m.width IS NOT NULL))"
}

fn parameters(request: &SearchRequest) -> [rusqlite::types::Value; 7] {
    use rusqlite::types::Value;
    let nullable = |value: Option<&String>| value.cloned().map(Value::Text).unwrap_or(Value::Null);
    [
        nullable(request.media_type.as_ref()),
        Value::Integer(request.favorites_only as i64),
        nullable(request.date_from.as_ref()),
        request
            .date_to
            .as_ref()
            .map(|date| Value::Text(format!("{date}T23:59:59.999999999Z")))
            .unwrap_or(Value::Null),
        nullable(request.camera.as_ref()),
        nullable(request.person_id.as_ref()),
        nullable(request.orientation.as_ref()),
    ]
}

fn order_sql(sort: &str, has_query: bool) -> &'static str {
    match sort {
        "name" => "m.filename COLLATE NOCASE ASC, m.id",
        "oldest" => "COALESCE(m.taken_at_override, m.taken_at, m.created_at) ASC, m.id",
        "newest" => "COALESCE(m.taken_at_override, m.taken_at, m.created_at) DESC, m.id",
        _ if has_query => "rank, m.id",
        _ => "COALESCE(m.taken_at_override, m.taken_at, m.created_at) DESC, m.id",
    }
}

fn exact_search(conn: &Connection, request: &SearchRequest) -> Result<MediaPage, String> {
    let terms = request
        .query
        .trim()
        .split_whitespace()
        .map(|word| word.replace('"', ""))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let has_query = !terms.is_empty();
    let query = format!("{}*", terms.join("* "));
    let from = if has_query {
        "FROM media_fts JOIN media_items m ON m.id = media_fts.media_id LEFT JOIN exif_data e ON e.media_id = m.id"
    } else {
        "FROM media_items m LEFT JOIN exif_data e ON e.media_id = m.id"
    };
    let where_clause = if has_query {
        format!("{} AND media_fts MATCH ?8", filter_sql())
    } else {
        filter_sql().to_string()
    };
    let args = parameters(request);
    let mut values = args.to_vec();
    if has_query {
        values.push(rusqlite::types::Value::Text(query));
    }
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) {from} WHERE {where_clause}"),
            rusqlite::params_from_iter(values.iter()),
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let order = order_sql(&request.sort, has_query);
    values.push(rusqlite::types::Value::Integer(
        request.limit.clamp(1, 200) as i64
    ));
    values.push(rusqlite::types::Value::Integer(request.offset.max(0)));
    let mut statement = conn
        .prepare(&format!(
            "SELECT m.id {from} WHERE {where_clause} ORDER BY {order} LIMIT ?{} OFFSET ?{}",
            values.len() - 1,
            values.len(),
        ))
        .map_err(|e| e.to_string())?;
    let ids = statement
        .query_map(rusqlite::params_from_iter(values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let items = ids
        .iter()
        .map(|id| row_to_media_item(conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(MediaPage { items, total })
}

fn semantic_search(state: &AppState, request: &SearchRequest) -> Result<MediaPage, String> {
    state.ai.touch();
    {
        let mut guard = state.ai.clip.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            if !crate::ai::model_manager::clip_models_ready(&state.app_data_dir) {
                return Err("AI models are not downloaded yet".into());
            }
            *guard = Some(
                crate::ai::clip::ClipModel::load(&crate::ai::model_manager::clip_dir(
                    &state.app_data_dir,
                ))
                .map_err(|e| e.to_string())?,
            );
        }
    }
    let query_vector = state
        .ai
        .clip
        .lock()
        .map_err(|e| e.to_string())?
        .as_mut()
        .unwrap()
        .embed_text(&request.query)
        .map_err(|e| e.to_string())?;
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut statement = conn.prepare(&format!(
        "SELECT m.id, embedding.vector, m.filename, COALESCE(m.taken_at_override, m.taken_at, m.created_at) FROM embeddings embedding
         JOIN media_items m ON m.id = embedding.media_id
         LEFT JOIN exif_data e ON e.media_id = m.id WHERE {}",
        filter_sql(),
    )).map_err(|e| e.to_string())?;
    let args = parameters(request);
    let rows = statement
        .query_map(rusqlite::params_from_iter(args.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut scored = rows
        .into_iter()
        .map(|(id, bytes, filename, date)| {
            let vector = crate::ai::clip::bytes_to_vector(&bytes);
            (
                id,
                crate::ai::clip::cosine_similarity(&query_vector, &vector),
                filename,
                date,
            )
        })
        .collect::<Vec<_>>();
    match request.sort.as_str() {
        "name" => scored.sort_by(|a, b| a.2.to_lowercase().cmp(&b.2.to_lowercase())),
        "oldest" => scored.sort_by(|a, b| a.3.cmp(&b.3)),
        "newest" => scored.sort_by(|a, b| b.3.cmp(&a.3)),
        _ => scored.sort_by(|a, b| b.1.total_cmp(&a.1)),
    }
    let total = scored.len() as i64;
    let start = (request.offset.max(0) as usize).min(scored.len());
    let end = (start + request.limit.clamp(1, 200) as usize).min(scored.len());
    let items = scored[start..end]
        .iter()
        .map(|(id, _, _, _)| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(MediaPage { items, total })
}

#[tauri::command]
pub fn list_search_cameras(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut statement = conn.prepare(
        "SELECT DISTINCT TRIM(COALESCE(e.camera_make, '') || ' ' || COALESCE(e.camera_model, '')) AS camera
         FROM exif_data e JOIN media_items m ON m.id = e.media_id
         WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND m.is_archived = 0 AND camera != ''
         ORDER BY camera COLLATE NOCASE",
    ).map_err(|e| e.to_string())?;
    let cameras = statement
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(cameras)
}

#[tauri::command]
pub fn search_library(
    state: State<'_, AppState>,
    request: SearchRequest,
) -> Result<MediaPage, String> {
    if request.semantic && !request.query.trim().is_empty() {
        semantic_search(&state, &request)
    } else {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        exact_search(&conn, &request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn exact_search_filters_before_paginating_and_excludes_private_items() {
        let db_path =
            std::env::temp_dir().join(format!("hive_search_fixture_{}.db", uuid::Uuid::new_v4()));
        let conn = crate::db::open(&db_path).unwrap();
        conn.execute("INSERT INTO folders (id, path, name, added_at) VALUES ('folder', 'fixture', 'Fixture', '2026-01-01')", []).unwrap();
        for index in 0..230 {
            let id = format!("image-{index:03}");
            conn.execute(
                "INSERT INTO media_items (id, folder_id, path, filename, hash, size, width, height, mime_type, media_type, created_at, modified_at, indexed_at, is_favorite, is_hidden, is_archived)
                 VALUES (?1, 'folder', ?2, ?3, 'hash', 100, 800, 600, 'image/jpeg', 'image', '2026-01-01', '2026-01-01', '2026-01-01', 1, ?4, ?5)",
                params![id, format!("fixture/{id}.jpg"), format!("cat-{index}.jpg"), (index == 0) as i64, (index == 1) as i64],
            ).unwrap();
            conn.execute(
                "INSERT INTO media_fts (media_id, filename) VALUES (?1, ?2)",
                params![id, format!("cat-{index}.jpg")],
            )
            .unwrap();
        }
        let request = SearchRequest {
            query: "cat".into(),
            semantic: false,
            media_type: Some("image".into()),
            favorites_only: true,
            date_from: None,
            date_to: None,
            camera: None,
            person_id: None,
            orientation: Some("landscape".into()),
            sort: "relevance".into(),
            limit: 100,
            offset: 0,
        };
        let first = exact_search(&conn, &request).unwrap();
        assert_eq!(first.total, 228);
        assert_eq!(first.items.len(), 100);
        assert!(first
            .items
            .iter()
            .all(|item| !item.is_hidden && !item.is_archived));
        let second = exact_search(
            &conn,
            &SearchRequest {
                offset: 100,
                ..request
            },
        )
        .unwrap();
        assert_eq!(second.items.len(), 100);
        assert!(first
            .items
            .iter()
            .all(|item| second.items.iter().all(|next| next.id != item.id)));
        drop(conn);
        let _ = std::fs::remove_file(db_path);
    }
}
