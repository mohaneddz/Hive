//! Blur detection, measured rather than predicted.
//!
//! The method is the variance of the Laplacian: run an edge-detection kernel
//! over the greyscale image and look at how much the result varies. A sharp
//! photo has strong, varied edges and therefore a high variance; a blurred one
//! has soft edges everywhere and a low one. It is a classic of image processing,
//! predates machine learning by decades, and needs no model.

use crate::commands::media::{row_to_media_item, scope_predicate};
use crate::models::{BlurReport, BlurryItem};
use crate::state::AppState;
use image::GrayImage;
use rusqlite::params;
use tauri::State;

/// Below this, a photo reads as soft. Calibrated on 8-bit greyscale, where a
/// crisp photo typically scores in the hundreds and a smeared one in the tens.
pub const DEFAULT_BLUR_THRESHOLD: f64 = 100.0;

/// Variance of the Laplacian over a greyscale image.
///
/// The kernel is the standard 4-neighbour Laplacian:
/// ```text
///      0   1   0
///      1  -4   1
///      0   1   0
/// ```
/// Border pixels are skipped — they have no full neighbourhood, and including
/// them would invent edges at the frame.
pub fn laplacian_variance(image: &GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 3 || height < 3 {
        return 0.0;
    }

    let at = |x: u32, y: u32| image.get_pixel(x, y)[0] as f64;

    let mut sum = 0.0;
    let mut sum_squares = 0.0;
    let mut count = 0.0;

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let response =
                at(x, y - 1) + at(x - 1, y) - 4.0 * at(x, y) + at(x + 1, y) + at(x, y + 1);
            sum += response;
            sum_squares += response * response;
            count += 1.0;
        }
    }

    let mean = sum / count;
    (sum_squares / count) - mean * mean
}

/// Scores every indexed photo and records the result.
///
/// Reads the 800px thumbnail rather than the original: sharpness survives that
/// downscale, and a 50-megapixel original would take a hundred times longer for
/// the same verdict. Scores are stored, so a second run is instant for anything
/// already measured.
#[tauri::command]
pub fn scan_blur(
    state: State<'_, AppState>,
    threshold: Option<f64>,
    rescan: Option<bool>,
) -> Result<BlurReport, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let threshold = threshold.unwrap_or(DEFAULT_BLUR_THRESHOLD).max(1.0);
    let force = rescan.unwrap_or(false);

    let live = scope_predicate(None);
    let pending: Vec<(String, Option<String>, String)> = conn
        .prepare(&format!(
            "SELECT m.id, t.path, m.path FROM media_items m
             LEFT JOIN thumbnails t ON t.media_id = m.id AND t.size = 'md'
             WHERE {live} AND m.media_type = 'image'
               AND (?1 = 1 OR m.blur_score IS NULL)"
        ))
        .map_err(|e| e.to_string())?
        .query_map(params![force as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    let mut scanned = 0i64;
    for (media_id, thumbnail_path, original_path) in pending {
        // Fall back to the original when no thumbnail was generated.
        let source = thumbnail_path.unwrap_or(original_path);
        let Ok(image) = image::open(&source) else {
            continue;
        };
        let score = laplacian_variance(&image.to_luma8());

        conn.execute(
            "UPDATE media_items SET blur_score = ?1 WHERE id = ?2",
            params![score, media_id],
        )
        .map_err(|e| e.to_string())?;
        scanned += 1;
    }

    let measured: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM media_items WHERE {live} AND blur_score IS NOT NULL"),
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Softest first — the worst offenders are what you came here for.
    let rows: Vec<(String, f64)> = conn
        .prepare(&format!(
            "SELECT id, blur_score FROM media_items
             WHERE {live} AND blur_score IS NOT NULL AND blur_score < ?1
             ORDER BY blur_score ASC LIMIT 200"
        ))
        .map_err(|e| e.to_string())?
        .query_map(params![threshold], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    let items = rows
        .into_iter()
        .map(|(id, score)| {
            row_to_media_item(&conn, &id)
                .map(|item| BlurryItem { item, score })
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(BlurReport {
        scanned,
        measured,
        threshold,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma};

    fn checkerboard(size: u32, cell: u32) -> GrayImage {
        ImageBuffer::from_fn(size, size, |x, y| {
            let on = (x / cell + y / cell) % 2 == 0;
            Luma([if on { 255u8 } else { 0u8 }])
        })
    }

    #[test]
    fn a_flat_image_has_no_variance() {
        let flat: GrayImage = ImageBuffer::from_pixel(32, 32, Luma([128]));
        assert!(laplacian_variance(&flat) < 0.001);
    }

    #[test]
    fn sharp_edges_score_far_above_soft_ones() {
        let sharp = checkerboard(64, 8);

        // Blurring the same pattern is exactly what a soft photo looks like.
        let blurred = image::DynamicImage::ImageLuma8(checkerboard(64, 8))
            .blur(3.0)
            .to_luma8();

        let sharp_score = laplacian_variance(&sharp);
        let blurred_score = laplacian_variance(&blurred);
        assert!(
            sharp_score > blurred_score * 5.0,
            "sharp {sharp_score} should dwarf blurred {blurred_score}"
        );
    }

    #[test]
    fn a_sharp_photo_clears_the_default_threshold() {
        assert!(laplacian_variance(&checkerboard(64, 8)) > DEFAULT_BLUR_THRESHOLD);
    }

    #[test]
    fn images_too_small_to_have_a_neighbourhood_score_zero() {
        let tiny: GrayImage = ImageBuffer::from_pixel(2, 2, Luma([200]));
        assert_eq!(laplacian_variance(&tiny), 0.0);
    }
}
