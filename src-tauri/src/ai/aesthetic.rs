//! Aesthetic scoring with the LAION aesthetic predictor.
//!
//! The predictor is a single linear layer over a CLIP embedding, trained on
//! photographs rated by people. Because CLIP has already described every indexed
//! photo as 512 numbers — and those numbers are sitting in the `embeddings`
//! table — scoring is a dot product:
//!
//! ```text
//!     score = bias + Σ (embedding[i] × weight[i])
//! ```
//!
//! No model file, no download, no image to decode. Re-scoring the whole library
//! costs a few milliseconds instead of a full pass over every JPEG.

use crate::ai::aesthetic_weights::{BIAS, WEIGHTS};

/// Raw predictor output for a photo, on roughly the 1–10 AVA scale.
///
/// `embedding` must be L2-normalised — the predictor was trained that way, and
/// feeding it an unnormalised vector silently shifts every score.
/// `ClipModel::embed_image` already normalises.
pub fn score_embedding(embedding: &[f32]) -> Option<f64> {
    if embedding.len() != WEIGHTS.len() {
        return None;
    }

    let dot: f32 = embedding
        .iter()
        .zip(WEIGHTS.iter())
        .map(|(value, weight)| value * weight)
        .sum();

    // In practice the predictor lands within 1..10, but nothing in a linear
    // layer enforces that, so the range is clamped rather than assumed.
    Some(((dot + BIAS) as f64).clamp(1.0, 10.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrongly_sized_embedding_is_refused_rather_than_scored() {
        assert!(score_embedding(&[0.0; 128]).is_none());
        assert!(score_embedding(&[]).is_none());
    }

    #[test]
    fn a_zero_embedding_scores_the_bias() {
        // Not a real embedding, but it isolates the bias term.
        let score = score_embedding(&[0.0; 512]).unwrap();
        assert!((score - BIAS as f64).abs() < 1e-4, "got {score}");
    }

    #[test]
    fn scores_stay_inside_the_ava_scale() {
        // An embedding aligned with the weights would blow past 10 unclamped.
        let aligned: Vec<f32> = WEIGHTS.iter().map(|w| w.signum()).collect();
        let opposed: Vec<f32> = WEIGHTS.iter().map(|w| -w.signum()).collect();

        assert_eq!(score_embedding(&aligned).unwrap(), 10.0);
        assert_eq!(score_embedding(&opposed).unwrap(), 1.0);
    }

    /// Needs a real library, so it is skipped by default. Run it with
    /// `cargo test --lib aesthetic -- --include-ignored --nocapture`.
    ///
    /// The weights were lifted out of LAION's `.pth` archive by hand, without
    /// PyTorch. A slip there — a wrong offset, the wrong dtype, a transposed
    /// slice — would still yield numbers between 1 and 10, because the result is
    /// clamped, and every synthetic test above would still pass. What a broken
    /// extraction cannot fake is a believable *spread* over real pictures: it
    /// collapses everything onto one value or pins it against the clamps.
    #[test]
    #[ignore]
    fn real_photographs_score_across_the_middle_of_the_scale() {
        use crate::ai::clip::bytes_to_vector;

        let Some(db) = std::env::var_os("APPDATA")
            .map(|base| std::path::Path::new(&base).join("com.hive").join("hive.db"))
        else {
            return;
        };
        if !db.is_file() {
            return;
        }

        let conn = rusqlite::Connection::open_with_flags(
            &db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("library opens");
        let mut stmt = conn
            .prepare(
                "SELECT m.filename, e.vector FROM embeddings e
                 JOIN media_items m ON m.id = e.media_id LIMIT 500",
            )
            .unwrap();
        let mut rated: Vec<(f64, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))
            .unwrap()
            .filter_map(|row| row.ok())
            .filter_map(|(name, bytes)| {
                score_embedding(&bytes_to_vector(&bytes)).map(|score| (score, name))
            })
            .collect();

        if rated.len() < 5 {
            return;
        }
        rated.sort_by(|a, b| b.0.total_cmp(&a.0));

        // Printed rather than asserted: whether the order is *sensible* is a
        // judgement no assertion can make, and it is the thing worth eyeballing
        // after touching the weights.
        for (score, name) in rated.iter().take(3) {
            println!("  best  {score:.2}  {name}");
        }
        for (score, name) in rated.iter().rev().take(3) {
            println!("  worst {score:.2}  {name}");
        }

        let scores: Vec<f64> = rated.iter().map(|(score, _)| *score).collect();

        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let spread =
            (scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64).sqrt();
        let low = scores.iter().copied().fold(f64::INFINITY, f64::min);
        let high = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!(
            "{} photos — mean {mean:.2}, spread {spread:.2}, range {low:.2}..{high:.2}",
            scores.len()
        );

        assert!(
            (2.5..=8.0).contains(&mean),
            "mean {mean:.2} is nowhere photographs sit on this scale"
        );
        assert!(
            spread > 0.15,
            "every photo scored alike (spread {spread:.3}) — the weights are inert"
        );
        assert!(
            scores.iter().filter(|s| **s <= 1.0 || **s >= 10.0).count() * 4 < scores.len(),
            "too many scores pinned against the clamps"
        );
    }

    #[test]
    fn a_realistic_normalised_embedding_lands_in_a_plausible_range() {
        // Spread evenly across all 512 dimensions, L2-normalised.
        let value = 1.0 / (512.0f32).sqrt();
        let score = score_embedding(&[value; 512]).unwrap();
        assert!((3.0..=7.0).contains(&score), "expected a mid-scale score, got {score}");
    }
}
