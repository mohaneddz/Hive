//! Best photo selection: pick the strongest image out of a group.
//!
//! Three signals, all of them already computed by other modules — nothing is
//! downloaded and nothing is inferred here:
//! - `blur_score` from quality.rs — higher means sharper
//! - `aesthetic_score` from aesthetic.rs — how good the shot looks
//! - CLIP embeddings from clip.rs — the image closest to the group's centre is
//!   the most representative of what the group is about
//!
//! Every signal is rescaled within the group before the weights are applied. That
//! is what makes the weights mean what they say: see [`normalise`].

use crate::ai::clip::{bytes_to_vector, cosine_similarity};
use crate::commands::media::row_to_media_item;
use crate::models::MediaItem;
use crate::state::AppState;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

/// How much each signal counts once they are all on the same scale.
///
/// Sharpness leads because a soft frame is unusable whatever else it has going
/// for it. Representativeness comes last: across a burst of near-identical shots
/// it barely separates anything, and it is there for the case the group is not a
/// burst at all.
const SHARPNESS_WEIGHT: f64 = 0.45;
const AESTHETIC_WEIGHT: f64 = 0.35;
const REPRESENTATIVENESS_WEIGHT: f64 = 0.20;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BestPhotoResult {
    pub best_media_id: String,
    pub item: MediaItem,
    pub score: f64,
    pub rankings: Vec<PhotoRanking>,
}

/// One photo's card. Every score is 0..1 and relative to this group: 1 means
/// "best of these photos", not "good in absolute terms".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoRanking {
    pub media_id: String,
    pub sharpness_score: f64,
    pub aesthetic_score: f64,
    pub representativeness_score: f64,
    pub total_score: f64,
}

/// What is known about one photo before any rescaling. `None` means the signal
/// was never computed for it.
#[derive(Debug, Clone)]
struct Signals {
    media_id: String,
    sharpness: Option<f64>,
    aesthetic: Option<f64>,
    representativeness: Option<f64>,
}

/// Rescales a signal so the group's weakest reads 0 and its strongest reads 1.
///
/// Without this the weights are fiction. Sharpness already spans its whole range
/// by construction, while cosine similarity between photos of the same moment
/// sits between roughly 0.85 and 0.98 — a spread of 0.13 against a spread of 1.
/// Multiplying those by 0.6 and 0.4 gives an influence of 0.60 against 0.05, so
/// the "40%" signal decided about 8% of the outcome.
///
/// A signal every photo shares equally separates nothing, so it returns the same
/// value for all of them rather than inventing an order.
fn normalise(values: &[Option<f64>]) -> Vec<Option<f64>> {
    let present: Vec<f64> = values.iter().filter_map(|value| *value).collect();
    if present.is_empty() {
        return vec![None; values.len()];
    }

    let min = present.iter().copied().fold(f64::INFINITY, f64::min);
    let max = present.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;

    values
        .iter()
        .map(|value| {
            value.map(|value| {
                if range <= f64::EPSILON {
                    0.5
                } else {
                    (value - min) / range
                }
            })
        })
        .collect()
}

/// Averages the signals that exist, weighted.
///
/// A missing signal abstains rather than voting an average: filling it with 0.5
/// is a claim about the photo that nothing supports, and it is how the photo that
/// was actually measured could end up ranked below the ones that were not.
fn combine(signals: &[(f64, Option<f64>)]) -> f64 {
    let mut weighted = 0.0;
    let mut total_weight = 0.0;
    for (weight, value) in signals {
        if let Some(value) = value {
            weighted += weight * value;
            total_weight += weight;
        }
    }
    if total_weight > 0.0 {
        weighted / total_weight
    } else {
        0.5
    }
}

/// Scores and orders the group. Pure, so the weighting can be checked without a
/// database behind it.
fn rank(signals: &[Signals]) -> Vec<PhotoRanking> {
    let sharpness = normalise(
        &signals
            .iter()
            .map(|entry| entry.sharpness)
            .collect::<Vec<_>>(),
    );
    let aesthetic = normalise(
        &signals
            .iter()
            .map(|entry| entry.aesthetic)
            .collect::<Vec<_>>(),
    );
    let representativeness = normalise(
        &signals
            .iter()
            .map(|entry| entry.representativeness)
            .collect::<Vec<_>>(),
    );

    let mut rankings: Vec<PhotoRanking> = signals
        .iter()
        .enumerate()
        .map(|(i, entry)| PhotoRanking {
            media_id: entry.media_id.clone(),
            sharpness_score: sharpness[i].unwrap_or(0.0),
            aesthetic_score: aesthetic[i].unwrap_or(0.0),
            representativeness_score: representativeness[i].unwrap_or(0.0),
            total_score: combine(&[
                (SHARPNESS_WEIGHT, sharpness[i]),
                (AESTHETIC_WEIGHT, aesthetic[i]),
                (REPRESENTATIVENESS_WEIGHT, representativeness[i]),
            ]),
        })
        .collect();

    // Ties break on the id so the same group always names the same winner.
    rankings.sort_by(|a, b| {
        b.total_score
            .total_cmp(&a.total_score)
            .then_with(|| a.media_id.cmp(&b.media_id))
    });
    rankings
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}

/// Reads both stored scores for the whole group in one query.
fn load_scores(
    conn: &Connection,
    media_ids: &[String],
) -> Result<HashMap<String, (Option<f64>, Option<f64>)>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, blur_score, aesthetic_score FROM media_items WHERE id IN ({})",
            placeholders(media_ids.len())
        ))
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params_from_iter(media_ids.iter()), |r| {
            Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?)))
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| e.to_string())
}

fn load_embeddings(
    conn: &Connection,
    media_ids: &[String],
) -> Result<HashMap<String, Vec<f32>>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT media_id, vector FROM embeddings WHERE media_id IN ({})",
            placeholders(media_ids.len())
        ))
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params_from_iter(media_ids.iter()), |r| {
            let id: String = r.get(0)?;
            let bytes: Vec<u8> = r.get(1)?;
            Ok((id, bytes_to_vector(&bytes)))
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|e| e.to_string())
}

/// The group's average direction: what these photos have in common.
fn centroid(embeddings: &HashMap<String, Vec<f32>>) -> Option<Vec<f32>> {
    let first = embeddings.values().next()?;
    let mut sum = vec![0.0f32; first.len()];
    for embedding in embeddings.values() {
        for (slot, value) in sum.iter_mut().zip(embedding.iter()) {
            *slot += value;
        }
    }

    let count = embeddings.len() as f32;
    let mean: Vec<f32> = sum.into_iter().map(|value| value / count).collect();
    // Back onto the unit sphere, so comparing against it is a plain cosine.
    let norm = mean.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        Some(mean.into_iter().map(|value| value / norm).collect())
    } else {
        None
    }
}

/// Picks the best photo from a group of media IDs.
///
/// Scores are relative to the group: the winner is the best of *these* photos,
/// which is the question worth asking when deciding which frame of a burst to
/// keep. A group of uniformly poor photos still has a winner.
#[tauri::command]
pub fn select_best_photo(
    state: State<'_, AppState>,
    media_ids: Vec<String>,
) -> Result<BestPhotoResult, String> {
    if media_ids.is_empty() {
        return Err("No media IDs provided".to_string());
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let scores = load_scores(&conn, &media_ids)?;
    let embeddings = load_embeddings(&conn, &media_ids)?;
    let centre = centroid(&embeddings);

    let signals: Vec<Signals> = media_ids
        .iter()
        .map(|id| {
            let (blur, aesthetic) = scores.get(id).copied().unwrap_or((None, None));
            Signals {
                media_id: id.clone(),
                sharpness: blur,
                aesthetic,
                representativeness: match (&centre, embeddings.get(id)) {
                    (Some(centre), Some(embedding)) => {
                        Some(cosine_similarity(embedding, centre) as f64)
                    }
                    _ => None,
                },
            }
        })
        .collect();

    let rankings = rank(&signals);
    let best = rankings.first().ok_or("No media IDs provided")?;
    let item = row_to_media_item(&conn, &best.media_id).map_err(|e| e.to_string())?;

    Ok(BestPhotoResult {
        best_media_id: best.media_id.clone(),
        score: best.total_score,
        item,
        rankings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(entries: &[(&str, Option<f64>, Option<f64>, Option<f64>)]) -> Vec<Signals> {
        entries
            .iter()
            .map(|(id, sharpness, aesthetic, representativeness)| Signals {
                media_id: (*id).to_string(),
                sharpness: *sharpness,
                aesthetic: *aesthetic,
                representativeness: *representativeness,
            })
            .collect()
    }

    #[test]
    fn a_narrow_signal_is_stretched_to_the_full_range() {
        // Cosine similarities between shots of the same moment. Left as they are,
        // a 0.13 spread cannot compete with sharpness spanning a full 1.0.
        let stretched = normalise(&[Some(0.85), Some(0.98), Some(0.90)]);

        assert_eq!(stretched[0], Some(0.0));
        assert_eq!(stretched[1], Some(1.0));
        assert!((stretched[2].unwrap() - 0.3846).abs() < 0.001);
    }

    #[test]
    fn a_signal_that_is_the_same_everywhere_ranks_nobody() {
        let flat = normalise(&[Some(120.0), Some(120.0)]);
        assert_eq!(flat, vec![Some(0.5), Some(0.5)]);
    }

    #[test]
    fn the_only_measured_photo_is_not_punished_for_being_measured() {
        // The old code gave the single measured photo (max == min) a sharpness of
        // 0.0 and handed every unmeasured one a default of 0.5, so measuring a
        // photo could only ever cost it the group.
        let ranked = rank(&signals(&[
            ("measured", Some(500.0), None, None),
            ("unknown-a", None, None, None),
            ("unknown-b", None, None, None),
        ]));

        assert_eq!(ranked[0].media_id, "measured");
    }

    #[test]
    fn the_sharpest_wins_when_nothing_else_separates_them() {
        let ranked = rank(&signals(&[
            ("soft", Some(40.0), Some(6.0), Some(0.9)),
            ("sharp", Some(900.0), Some(6.0), Some(0.9)),
        ]));

        assert_eq!(ranked[0].media_id, "sharp");
        assert_eq!(ranked[0].sharpness_score, 1.0);
    }

    #[test]
    fn among_equally_sharp_frames_the_better_looking_one_wins() {
        // A burst: two frames were missed, two are sharp. Rescaling puts the two
        // sharp ones within 1% of each other, which leaves the aesthetic score to
        // separate them — the signal the old weighting never consulted at all.
        let ranked = rank(&signals(&[
            ("missed-a", Some(40.0), Some(5.0), Some(0.93)),
            ("missed-b", Some(50.0), Some(5.0), Some(0.93)),
            ("sharp-plain", Some(910.0), Some(4.0), Some(0.93)),
            ("sharp-pretty", Some(900.0), Some(8.5), Some(0.93)),
        ]));

        assert_eq!(ranked[0].media_id, "sharp-pretty");
        assert_eq!(ranked[1].media_id, "sharp-plain");
    }

    #[test]
    fn only_two_photos_makes_the_rescaling_absolute() {
        // Worth pinning down: with a group of two, min-max always lands on 0 and
        // 1 however close the pair really is, so the highest-weighted signal
        // simply wins. Meaningful separation needs a group with a spread in it.
        let ranked = rank(&signals(&[
            ("barely-sharper", Some(910.0), Some(4.0), Some(0.93)),
            ("far-prettier", Some(900.0), Some(8.5), Some(0.93)),
        ]));

        assert_eq!(ranked[0].media_id, "barely-sharper");
    }

    #[test]
    fn representativeness_still_decides_when_it_is_the_only_signal() {
        let ranked = rank(&signals(&[
            ("odd-one-out", None, None, Some(0.71)),
            ("typical", None, None, Some(0.95)),
        ]));

        assert_eq!(ranked[0].media_id, "typical");
    }

    #[test]
    fn a_group_with_nothing_computed_still_returns_an_order() {
        let ranked = rank(&signals(&[("a", None, None, None), ("b", None, None, None)]));

        assert_eq!(ranked.len(), 2);
        assert!(ranked.iter().all(|entry| entry.total_score == 0.5));
        // Tie broken on the id, so the answer does not wander between runs.
        assert_eq!(ranked[0].media_id, "a");
    }
}
