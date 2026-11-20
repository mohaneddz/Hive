use crate::ai::llm::ChatModel;
use crate::jobs;
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub answer: String,
    pub media_ids: Vec<String>,
}

const SYSTEM_PROMPT: &str = "You are Hive, a local assistant for someone's personal photo \
library. You are given a short list of photos (filename, date, camera, and any text found in \
the photo, plus captions, tags, people, and places where available) that were retrieved for \
the user's question. Treat those records as evidence. Answer using only what's in that list \
— if it doesn't contain the answer, say so plainly rather than guessing. Keep replies to a \
few sentences, in a warm and direct tone, and don't repeat the raw list back verbatim.";

#[tauri::command]
pub async fn download_llm_model(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.ai.touch();
    let app_data_dir = state.app_data_dir.clone();
    let db_path = state.db_path.clone();
    let ai = state.ai.clone();

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "download_llm_model").map_err(|e| e.to_string())?;

    let app_for_progress = app.clone();
    let db_path_for_progress = db_path.clone();
    let job_id_for_progress = job_id.clone();
    let result = crate::ai::model_manager::ensure_llm_models(&app_data_dir, move |done, total| {
        if let Ok(conn) = crate::db::open(&db_path_for_progress) {
            jobs::emit_progress(
                &app_for_progress,
                &conn,
                &job_id_for_progress,
                "download_llm_model",
                "running",
                done as i64,
                total as i64,
                None,
            );
        }
    })
    .await;

    match result {
        Ok(()) => {
            jobs::emit_progress(&app, &conn, &job_id, "download_llm_model", "completed", 1, 1, None);
            match ChatModel::load(&crate::ai::model_manager::llm_dir(&app_data_dir)) {
                Ok(model) => {
                    *ai.llm.lock().unwrap() = Some(model);
                    Ok(())
                }
                Err(e) => Err(format!("Model downloaded but failed to load: {e}")),
            }
        }
        Err(e) => {
            jobs::fail_job(&conn, &job_id, &e.to_string());
            Err(e.to_string())
        }
    }
}

fn ensure_llm_loaded(state: &State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.ai.llm.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    if !crate::ai::model_manager::llm_models_ready(&state.app_data_dir) {
        return Err("The local chat model is not downloaded yet".to_string());
    }
    let model = ChatModel::load(&crate::ai::model_manager::llm_dir(&state.app_data_dir))
        .map_err(|e| e.to_string())?;
    *guard = Some(model);
    Ok(())
}

const CHAT_PHOTO_LIMIT: usize = 8;
const CATALOG: &str = "
    SELECT m.id, m.filename, COALESCE(m.taken_at_override, m.taken_at) AS taken_at,
           e.camera_make, e.camera_model, m.title, m.description, c.text AS caption,
           (SELECT GROUP_CONCAT(DISTINCT t.tag) FROM tags t WHERE t.media_id = m.id) AS tags,
           (SELECT GROUP_CONCAT(DISTINCT p.name)
              FROM faces f JOIN people p ON p.id = f.person_id WHERE f.media_id = m.id) AS people,
           COALESCE(area.name, city.name, region.name) AS place
      FROM media_items m
      LEFT JOIN exif_data e ON e.media_id = m.id
      LEFT JOIN captions c ON c.media_id = m.id
      LEFT JOIN geocode_cache area ON area.lat = ROUND(e.gps_lat, 2) AND area.lon = ROUND(e.gps_lon, 2)
      LEFT JOIN geocode_cache city ON city.lat = ROUND(e.gps_lat, 1) AND city.lon = ROUND(e.gps_lon, 1)
      LEFT JOIN geocode_cache region ON region.lat = ROUND(e.gps_lat, 0) AND region.lon = ROUND(e.gps_lon, 0)
     WHERE m.media_type = 'image' AND m.is_trashed = 0 AND m.is_hidden = 0 AND m.is_archived = 0";

fn retrieve_text_ids(conn: &Connection, query: &str) -> rusqlite::Result<Vec<String>> {
    let mut seen = HashSet::new();
    let terms: Vec<String> = query.split(|ch: char| !ch.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .filter(|term| !matches!(term.as_str(),
            "a" | "an" | "and" | "are" | "can" | "do" | "find" | "for" | "from" | "i" | "in" |
            "is" | "me" | "my" | "of" | "on" | "or" | "photo" | "photos" | "show" | "the" |
            "to" | "was" | "what" | "when" | "where" | "which" | "who" | "with" | "you"))
        .filter(|term| seen.insert(term.clone()))
        .take(24)
        .collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    // Look for terms across multiple fields, including names and captions that CLIP cannot see.
    let terms_json = serde_json::json!(terms).to_string();
    let fts_query = terms.iter().map(|term| format!("\"{term}\"*")).collect::<Vec<_>>().join(" OR ");
    let mut stmt = conn.prepare(&format!("
        WITH catalog AS MATERIALIZED ({CATALOG}),
             terms AS (SELECT value AS term FROM json_each(?1)),
             fts AS MATERIALIZED (
                 SELECT m.id, MIN(media_fts.rank) AS rank
                   FROM media_fts JOIN media_items m ON m.id = media_fts.media_id
                  WHERE media_fts MATCH ?2 AND m.media_type = 'image'
                    AND m.is_trashed = 0 AND m.is_hidden = 0 AND m.is_archived = 0
                  GROUP BY m.id ORDER BY rank LIMIT ?3
             ),
             matches AS (
                 SELECT id, SUM(CASE
                    WHEN instr(lower(concat_ws(' ', people, place)), term) > 0 THEN 4
                    WHEN instr(lower(concat_ws(' ', tags, caption, title, description)), term) > 0 THEN 3
                    WHEN instr(lower(concat_ws(' ', filename, taken_at, camera_make, camera_model)), term) > 0 THEN 2
                    ELSE 0 END) AS score
                   FROM catalog CROSS JOIN terms GROUP BY id HAVING score > 0
             )
        SELECT ids.id FROM (SELECT id FROM matches UNION SELECT id FROM fts) ids
        LEFT JOIN matches ON matches.id = ids.id LEFT JOIN fts ON fts.id = ids.id
        ORDER BY COALESCE(matches.score, 0) DESC, fts.rank NULLS LAST, ids.id LIMIT ?3"))?;
    let ids = stmt.query_map(params![terms_json, fts_query, CHAT_PHOTO_LIMIT], |row| row.get(0))?;
    ids.collect()
}

struct ContextRow {
    id: String,
    filename: String,
    taken_at: Option<String>,
    camera_model: Option<String>,
    ocr_text: Option<String>,
    caption: Option<String>,
    tags: Option<String>,
    people: Option<String>,
    place: Option<String>,
    title: Option<String>,
    description: Option<String>,
}

fn load_context(conn: &Connection, media_ids: &[String]) -> rusqlite::Result<Vec<ContextRow>> {
    let mut stmt = conn.prepare(&format!("
        WITH selected AS (SELECT key, value FROM json_each(?1)),
             catalog AS ({CATALOG}),
             ocr AS (SELECT media_id, MAX(ocr_text) AS text FROM media_fts
                     WHERE media_id IN (SELECT value FROM selected) GROUP BY media_id)
        SELECT c.id, c.filename, c.taken_at, NULLIF(concat_ws(' ', c.camera_make, c.camera_model), ''),
               ocr.text, c.caption, c.tags, c.people, c.place, c.title, c.description
          FROM selected JOIN catalog c ON c.id = selected.value LEFT JOIN ocr ON ocr.media_id = c.id
         ORDER BY selected.key"))?;
    let ids_json = serde_json::json!(media_ids).to_string();
    let rows = stmt.query_map([ids_json], |row| Ok(ContextRow {
        id: row.get(0)?,
        filename: row.get(1)?,
        taken_at: row.get(2)?,
        camera_model: row.get(3)?,
        ocr_text: row.get(4)?,
        caption: row.get(5)?,
        tags: row.get(6)?,
        people: row.get(7)?,
        place: row.get(8)?,
        title: row.get(9)?,
        description: row.get(10)?,
    }))?;
    rows.collect()
}

fn format_context(rows: &[ContextRow]) -> String {
    if rows.is_empty() {
        return "(no matching photos were found in the library)".to_string();
    }
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let date = row.taken_at.as_deref().unwrap_or("unknown date");
            let camera = row.camera_model.as_deref().unwrap_or("unknown camera");
            let mut line = format!("{}. {} (taken {}, {})", i + 1, row.filename, date, camera);
            for (label, value) in [
                ("title", &row.title), ("description", &row.description), ("visible text", &row.ocr_text),
                ("caption", &row.caption), ("tags", &row.tags), ("people", &row.people), ("place", &row.place),
            ] {
                if let Some(value) = value.as_deref().map(str::trim).filter(|text| !text.is_empty()) {
                    use std::fmt::Write;
                    let excerpt: String = value.chars().take(200).collect();
                    let _ = write!(line, " — {label}: {excerpt}");
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tauri::command]
pub async fn gallery_chat(app: AppHandle, state: State<'_, AppState>, message: String) -> Result<ChatResponse, String> {
    state.ai.touch();
    ensure_llm_loaded(&state)?;

    let mut media_ids = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        retrieve_text_ids(&conn, &message).map_err(|e| e.to_string())?
    };
    if crate::ai::model_manager::clip_models_ready(&state.app_data_dir) && media_ids.len() < CHAT_PHOTO_LIMIT {
        if let Ok(items) = crate::commands::ai::semantic_search(state.clone(), message.clone(), CHAT_PHOTO_LIMIT as i64) {
            for item in items {
                if item.media_type == "image" && !media_ids.contains(&item.id) {
                    media_ids.push(item.id);
                }
                if media_ids.len() == CHAT_PHOTO_LIMIT { break; }
            }
        }
    }
    let rows = {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        load_context(&conn, &media_ids).map_err(|e| e.to_string())?
    };
    if rows.is_empty() {
        return Ok(ChatResponse { answer: "I couldn’t find any photos matching that question.".into(), media_ids: Vec::new() });
    }
    let context = format_context(&rows);
    let media_ids = rows.into_iter().map(|row| row.id).collect();

    let ai = state.ai.clone();
    let db_path = state.db_path.clone();
    let user_prompt = format!("Photos found for this question:\n{context}\n\nQuestion: {message}");

    let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
    let job_id = jobs::create_job(&conn, "gallery_chat").map_err(|e| e.to_string())?;
    jobs::emit_progress(&app, &conn, &job_id, "gallery_chat", "running", 0, 1, None);

    let answer = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let mut guard = ai.llm.lock().unwrap();
        let model = guard.as_mut().ok_or("chat model not loaded")?;
        model.chat(SYSTEM_PROMPT, &user_prompt).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;

    match &answer {
        Ok(_) => jobs::emit_progress(&app, &conn, &job_id, "gallery_chat", "completed", 1, 1, None),
        Err(e) => jobs::fail_job(&conn, &job_id, e),
    }

    Ok(ChatResponse { answer: answer?, media_ids })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn chat_retrieves_enrichment_and_ocr_without_hidden_items() {
        let conn = crate::db::open(Path::new(":memory:")).unwrap();
        conn.execute_batch("
            INSERT INTO folders (id, path, name, added_at) VALUES ('f', '/photos', 'Photos', 'today');
            INSERT INTO media_items
                (id, folder_id, path, filename, hash, size, mime_type, media_type, created_at, modified_at, indexed_at, is_hidden)
            VALUES ('match', 'f', '/photos/a.jpg', 'a.jpg', 'a', 1, 'image/jpeg', 'image', 'today', 'today', 'today', 0),
                   ('ocr', 'f', '/photos/b.jpg', 'b.jpg', 'b', 1, 'image/jpeg', 'image', 'today', 'today', 'today', 0),
                   ('hidden', 'f', '/photos/c.jpg', 'c.jpg', 'c', 1, 'image/jpeg', 'image', 'today', 'today', 'today', 1);
            INSERT INTO people (id, name, created_at) VALUES ('p', 'Sarah', 'today');
            INSERT INTO faces (id, media_id, person_id, x0, y0, x1, y1, embedding, created_at)
            VALUES ('face', 'match', 'p', 0, 0, 8, 8, X'00', 'today');
            INSERT INTO captions (media_id, text, model, created_at)
            VALUES ('match', 'A bowl of pasta', 'fixture', 'today'), ('hidden', 'Sarah eating pasta', 'fixture', 'today');
            INSERT INTO tags (id, media_id, tag, confidence, created_at) VALUES ('t', 'match', 'dinner', 1, 'today');
            INSERT INTO exif_data (media_id, gps_lat, gps_lon) VALUES ('match', 41.932, 12.523);
            INSERT INTO geocode_cache (lat, lon, name, looked_up_at) VALUES (41.9, 12.5, 'Rome, Italy', 'today');
            INSERT INTO media_fts (media_id, filename, ocr_text) VALUES
                ('ocr', 'b.jpg', 'Pasta menu'), ('hidden', 'c.jpg', 'Sarah Italy pasta');
        ").unwrap();

        let ids = retrieve_text_ids(&conn, "Show me photos of Sarah eating pasta in Italy!").unwrap();
        assert_eq!(ids, ["match", "ocr"]);
        let rows = load_context(&conn, &[ids[1].clone(), ids[0].clone(), "hidden".into()]).unwrap();
        assert_eq!(rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(), ["ocr", "match"]);
        let context = format_context(&rows);
        for field in ["Pasta menu", "bowl of pasta", "Sarah", "Rome, Italy", "dinner"] {
            assert!(context.contains(field), "context missing {field}");
        }
    }
}
