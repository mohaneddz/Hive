//! Aesthetic ranking: score indexed photos and read back the ranking.
//!
//! Unlike the other AI features there is nothing to download. The predictor is a
//! linear layer over the CLIP embedding, and those embeddings are already in the
//! database — so scoring reads numbers, never pixels.

use crate::ai::aesthetic::score_embedding;
use crate::ai::clip::bytes_to_vector;
use crate::commands::media::row_to_media_item;
use crate::jobs;
use crate::models::MediaItem;
use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedItem {
    pub item: MediaItem,
    pub aesthetic_score: f64,
}

/// Scores every embedded photo that has no score yet.
///
/// The only prerequisite is a CLIP embedding, which the indexer already produces.
/// Photos indexed before CLIP was set up simply have none and are skipped —
/// running the CLIP backfill first is what brings them in.
#[tauri::command]
pub async fn backfill_aesthetic(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let db_path = state.db_path.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "aesthetic_backfill").map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT m.id, e.vector FROM media_items m
                 JOIN embeddings e ON e.media_id = m.id
                 WHERE m.is_trashed = 0 AND m.aesthetic_score IS NULL",
            )
            .map_err(|e| e.to_string())?;
        let pending: Vec<(String, Vec<u8>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let total = pending.len() as i64;
        for (i, (media_id, vector)) in pending.iter().enumerate() {
            if cancelled_jobs.lock().unwrap().remove(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(
                    &app, &conn, &job_id, "aesthetic_backfill", "cancelled",
                    i as i64, total, None,
                );
                return Ok(());
            }

            if let Some(score) = score_embedding(&bytes_to_vector(vector)) {
                let _ = conn.execute(
                    "UPDATE media_items SET aesthetic_score = ?1 WHERE id = ?2",
                    params![score, media_id],
                );
            }

            jobs::emit_progress(
                &app, &conn, &job_id, "aesthetic_backfill", "running",
                i as i64 + 1, total, None,
            );
        }

        jobs::emit_progress(&app, &conn, &job_id, "aesthetic_backfill", "completed", total, total, None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Returns the top-rated photos by aesthetic score.
#[tauri::command]
pub fn get_aesthetic_ranking(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<RankedItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, aesthetic_score FROM media_items
             WHERE aesthetic_score IS NOT NULL AND is_trashed = 0
             ORDER BY aesthetic_score DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, f64)> = stmt
        .query_map(params![limit], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    rows.iter()
        .map(|(id, score)| {
            let item = row_to_media_item(&conn, id).map_err(|e| e.to_string())?;
            Ok(RankedItem {
                item,
                aesthetic_score: *score,
            })
        })
        .collect()
}
