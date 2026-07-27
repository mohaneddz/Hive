//! Auto-tagging: classify images against a predefined tag vocabulary using CLIP.
//!
//! This reuses the CLIP model that Mohaned set up for semantic search. Instead of
//! comparing a user query to images, we compare each image to a fixed list of tag
//! descriptions ("a photo of a beach", "a photo of a dog", etc.) and keep the tags
//! whose cosine similarity exceeds a threshold.
//!
//! No new model is needed — the CLIP vision and text towers already do exactly this.

use crate::ai::clip::{cosine_similarity, bytes_to_vector, ClipModel};
use crate::ai::model_manager;
use crate::commands::media::row_to_media_item;
use crate::jobs;
use crate::models::MediaItem;
use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

/// Minimum cosine similarity for a tag to be assigned. Tuned for CLIP ViT-B/32:
/// 0.23 is low enough to catch relevant labels while filtering out noise.
const TAG_THRESHOLD: f32 = 0.23;

/// Maximum tags per image — keeps the results focused.
const MAX_TAGS_PER_IMAGE: usize = 8;

/// The predefined tag vocabulary. Each entry is (tag_name, CLIP prompt).
/// The prompt format "a photo of ..." matches CLIP's training distribution and
/// gives noticeably better results than bare keywords.
const TAG_VOCABULARY: &[(&str, &str)] = &[
    // Nature & Landscapes
    ("beach", "a photo of a beach with sand and ocean"),
    ("mountain", "a photo of mountains"),
    ("forest", "a photo of a forest with trees"),
    ("lake", "a photo of a lake"),
    ("river", "a photo of a river"),
    ("desert", "a photo of a desert"),
    ("sunset", "a photo of a sunset"),
    ("sunrise", "a photo of a sunrise"),
    ("sky", "a photo of a sky with clouds"),
    ("snow", "a photo of snow"),
    ("garden", "a photo of a garden with flowers"),
    ("field", "a photo of a field or meadow"),
    ("waterfall", "a photo of a waterfall"),
    // Animals
    ("dog", "a photo of a dog"),
    ("cat", "a photo of a cat"),
    ("bird", "a photo of a bird"),
    ("horse", "a photo of a horse"),
    ("fish", "a photo of fish"),
    ("insect", "a photo of an insect or butterfly"),
    // Food & Drink
    ("food", "a photo of food or a meal"),
    ("dessert", "a photo of a dessert or cake"),
    ("coffee", "a photo of coffee or tea"),
    ("fruit", "a photo of fruit"),
    ("restaurant", "a photo taken in a restaurant"),
    // People & Activities
    ("selfie", "a selfie photo"),
    ("group photo", "a group photo of people"),
    ("portrait", "a portrait photo of a person"),
    ("baby", "a photo of a baby"),
    ("wedding", "a photo of a wedding"),
    ("party", "a photo of a party or celebration"),
    ("sport", "a photo of sports or exercise"),
    ("concert", "a photo of a concert or music event"),
    ("graduation", "a photo of a graduation ceremony"),
    // Urban & Architecture
    ("city", "a photo of a city skyline"),
    ("street", "a photo of a street"),
    ("building", "a photo of a building or architecture"),
    ("bridge", "a photo of a bridge"),
    ("monument", "a photo of a monument or landmark"),
    ("church", "a photo of a church or mosque"),
    // Indoor
    ("interior", "a photo of a room interior"),
    ("office", "a photo of an office"),
    ("kitchen", "a photo of a kitchen"),
    // Transport
    ("car", "a photo of a car"),
    ("airplane", "a photo of an airplane"),
    ("boat", "a photo of a boat or ship"),
    ("train", "a photo of a train"),
    // Art & Objects
    ("art", "a photo of artwork or painting"),
    ("book", "a photo of a book"),
    ("flower", "a photo of a flower"),
    // Mood / Scene
    ("night", "a photo taken at night"),
    ("rain", "a photo of rain or rainy weather"),
    ("fireworks", "a photo of fireworks"),
    // Documents
    ("screenshot", "a screenshot of a computer or phone screen"),
    ("document", "a photo of a document or text"),
    ("handwriting", "a photo of handwritten text"),
    ("meme", "a meme or funny image with text"),
];

/// A single tag result for an image.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagResult {
    pub tag: String,
    pub confidence: f32,
}

/// Summary of a tag across the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSummary {
    pub tag: String,
    pub count: i64,
    pub cover_media_id: Option<String>,
}

/// Pre-computes the text embeddings for the entire tag vocabulary.
/// This is done once when tagging starts, then reused for every image.
fn compute_tag_embeddings(clip: &mut ClipModel) -> Vec<(&'static str, Vec<f32>)> {
    TAG_VOCABULARY
        .iter()
        .filter_map(|(tag, prompt)| {
            clip.embed_text(prompt).ok().map(|embed| (*tag, embed))
        })
        .collect()
}

/// Tags a single image against the pre-computed tag embeddings.
fn tag_image(
    image_embed: &[f32],
    tag_embeds: &[(&str, Vec<f32>)],
) -> Vec<TagResult> {
    let mut scored: Vec<TagResult> = tag_embeds
        .iter()
        .map(|(tag, embed)| TagResult {
            tag: tag.to_string(),
            confidence: cosine_similarity(image_embed, embed),
        })
        .filter(|r| r.confidence >= TAG_THRESHOLD)
        .collect();

    scored.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    scored.truncate(MAX_TAGS_PER_IMAGE);
    scored
}

/// Stores tags for a media item, replacing any previous auto-generated tags.
fn store_tags(conn: &Connection, media_id: &str, tags: &[TagResult]) -> rusqlite::Result<()> {
    // Remove old auto-tags (keep manual ones).
    conn.execute(
        "DELETE FROM tags WHERE media_id = ?1 AND source = 'auto'",
        params![media_id],
    )?;

    let now = chrono::Utc::now().to_rfc3339();
    for tag in tags {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tags (id, media_id, tag, confidence, source, created_at)
             VALUES (?1, ?2, ?3, ?4, 'auto', ?5)
             ON CONFLICT(media_id, tag) DO UPDATE SET
                confidence = excluded.confidence,
                created_at = excluded.created_at",
            params![id, media_id, tag.tag, tag.confidence, now],
        )?;
    }
    Ok(())
}

/// Returns the tags for a single media item.
#[tauri::command]
pub fn get_tags(state: State<'_, AppState>, media_id: String) -> Result<Vec<TagResult>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT tag, confidence FROM tags WHERE media_id = ?1 ORDER BY confidence DESC",
        )
        .map_err(|e| e.to_string())?;
    let tags = stmt
        .query_map(params![media_id], |r| {
            Ok(TagResult {
                tag: r.get(0)?,
                confidence: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(tags)
}

/// Lists all unique tags with their counts, sorted by count descending.
#[tauri::command]
pub fn list_all_tags(state: State<'_, AppState>) -> Result<Vec<TagSummary>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT t.tag, COUNT(*) as cnt,
                    (SELECT t2.media_id FROM tags t2
                     JOIN media_items m ON m.id = t2.media_id
                     WHERE t2.tag = t.tag AND m.is_trashed = 0
                     ORDER BY t2.confidence DESC LIMIT 1) as cover
             FROM tags t
             JOIN media_items m ON m.id = t.media_id
             WHERE m.is_trashed = 0
             GROUP BY t.tag
             ORDER BY cnt DESC",
        )
        .map_err(|e| e.to_string())?;
    let summaries = stmt
        .query_map([], |r| {
            Ok(TagSummary {
                tag: r.get(0)?,
                count: r.get(1)?,
                cover_media_id: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(summaries)
}

/// Returns all non-trashed media items that carry a given tag.
#[tauri::command]
pub fn list_media_by_tag(
    state: State<'_, AppState>,
    tag: String,
    limit: i64,
) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT m.id FROM media_items m
             JOIN tags t ON t.media_id = m.id
             WHERE t.tag = ?1 AND m.is_trashed = 0
             ORDER BY t.confidence DESC
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![tag, limit], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Backfill: auto-tag every image that has a CLIP embedding but no tags yet.
/// Runs as a cancellable background job, following the same pattern as
/// `backfill_embeddings` / `backfill_ocr` / `backfill_faces`.
#[tauri::command]
pub async fn backfill_tags(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Ensure CLIP is loaded.
    {
        let mut guard = state.ai.clip.lock().unwrap();
        if guard.is_none() {
            if !model_manager::clip_models_ready(&state.app_data_dir) {
                return Err("CLIP models are not downloaded yet".to_string());
            }
            let model = ClipModel::load(&model_manager::clip_dir(&state.app_data_dir))
                .map_err(|e| e.to_string())?;
            *guard = Some(model);
        }
    }

    let db_path = state.db_path.clone();
    let ai = state.ai.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "tag_backfill").map_err(|e| e.to_string())?;

        // Pre-compute tag embeddings (once).
        let tag_embeds = {
            let mut guard = ai.clip.lock().unwrap();
            let model = guard.as_mut().unwrap();
            compute_tag_embeddings(model)
        };

        if tag_embeds.is_empty() {
            jobs::emit_progress(&app, &conn, &job_id, "tag_backfill", "completed", 0, 0, None);
            return Ok(());
        }

        // Find images with embeddings but no auto-tags.
        let mut stmt = conn
            .prepare(
                "SELECT e.media_id, e.vector FROM embeddings e
                 JOIN media_items m ON m.id = e.media_id
                 WHERE m.is_trashed = 0
                   AND NOT EXISTS (SELECT 1 FROM tags t WHERE t.media_id = e.media_id AND t.source = 'auto')",
            )
            .map_err(|e| e.to_string())?;
        let pending: Vec<(String, Vec<u8>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let total = pending.len() as i64;
        for (i, (media_id, vector_bytes)) in pending.iter().enumerate() {
            if cancelled_jobs.lock().unwrap().remove(&job_id) {
                jobs::cancel_job(&conn, &job_id);
                jobs::emit_progress(
                    &app, &conn, &job_id, "tag_backfill", "cancelled",
                    i as i64, total, None,
                );
                return Ok(());
            }

            let image_embed = bytes_to_vector(vector_bytes);
            let tags = tag_image(&image_embed, &tag_embeds);
            let _ = store_tags(&conn, media_id, &tags);

            jobs::emit_progress(
                &app, &conn, &job_id, "tag_backfill", "running",
                i as i64 + 1, total, None,
            );
        }

        jobs::emit_progress(&app, &conn, &job_id, "tag_backfill", "completed", total, total, None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Tags a single image if it has a CLIP embedding. Called during indexing
/// to tag images as they come in, mirroring `try_embed_image` / `try_extract_ocr_text`.
pub fn try_auto_tag(
    ai: &crate::ai::AiState,
    conn: &Connection,
    media_id: &str,
) {
    // Need the CLIP model for tag embeddings.
    let mut guard = ai.clip.lock().unwrap();
    let Some(model) = guard.as_mut() else { return };

    // Read the image's existing CLIP embedding.
    let vector_bytes: Vec<u8> = match conn.query_row(
        "SELECT vector FROM embeddings WHERE media_id = ?1",
        params![media_id],
        |r| r.get(0),
    ) {
        Ok(v) => v,
        Err(_) => return,
    };

    let tag_embeds = compute_tag_embeddings(model);
    let image_embed = bytes_to_vector(&vector_bytes);
    let tags = tag_image(&image_embed, &tag_embeds);
    let _ = store_tags(conn, media_id, &tags);
}
