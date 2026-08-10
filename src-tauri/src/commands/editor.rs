use crate::commands::media::row_to_media_item;
use crate::commands::utilities::unique_destination;
use crate::models::{EditOps, MediaItem};
use crate::state::AppState;
use crate::{indexing, thumbnails};
use image::{DynamicImage, ImageFormat};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

/// Quality used when an edit is written back as JPEG. High enough that a single
/// save is visually lossless; re-encoding is still not free, which is why saving
/// a copy is the offered default.
const JPEG_QUALITY: u8 = 95;

/// The colour pipeline, applied to one pixel.
///
/// Order matters and is fixed: **brightness → contrast → saturation → grayscale
/// → sepia → temperature**. `applyColour` in the editor runs the exact same
/// steps in the exact same order, which is what keeps the live preview honest.
pub fn adjust_pixel(rgb: [f32; 3], ops: &EditOps) -> [f32; 3] {
    let mut channels = rgb;

    for value in &mut channels {
        *value *= ops.brightness;
        *value = (*value - 0.5) * ops.contrast + 0.5;
    }

    // Rec. 709 luma, the same coefficients the CSS saturate() filter uses.
    let luma = 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
    for value in &mut channels {
        *value = luma + (*value - luma) * ops.saturation;
    }

    if ops.grayscale > 0.0 {
        let grey = 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
        for value in &mut channels {
            *value += (grey - *value) * ops.grayscale;
        }
    }

    if ops.sepia > 0.0 {
        // The matrix from the CSS Filter Effects specification.
        let [r, g, b] = channels;
        let toned = [
            0.393 * r + 0.769 * g + 0.189 * b,
            0.349 * r + 0.686 * g + 0.168 * b,
            0.272 * r + 0.534 * g + 0.131 * b,
        ];
        for (index, value) in channels.iter_mut().enumerate() {
            *value += (toned[index] - *value) * ops.sepia;
        }
    }

    if ops.temperature != 0.0 {
        // Warm lifts red and drops blue; cool does the reverse. A third of the
        // channel at full strength is a visible shift that still looks like a
        // photograph.
        let shift = ops.temperature * 0.3;
        channels[0] *= 1.0 + shift;
        channels[2] *= 1.0 - shift;
    }

    channels
}

fn is_neutral(ops: &EditOps) -> bool {
    (ops.brightness - 1.0).abs() < f32::EPSILON
        && (ops.contrast - 1.0).abs() < f32::EPSILON
        && (ops.saturation - 1.0).abs() < f32::EPSILON
        && ops.grayscale.abs() < f32::EPSILON
        && ops.sepia.abs() < f32::EPSILON
        && ops.temperature.abs() < f32::EPSILON
}

fn adjust_colour(image: DynamicImage, ops: &EditOps) -> DynamicImage {
    if is_neutral(ops) {
        return image;
    }

    let mut buffer = image.to_rgba8();
    for pixel in buffer.pixels_mut() {
        let adjusted = adjust_pixel(
            [
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            ],
            ops,
        );
        for (index, value) in adjusted.iter().enumerate() {
            pixel[index] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    DynamicImage::ImageRgba8(buffer)
}

/// rotate → flip → crop → colour. Keep this in step with `applyOps` in the
/// editor page, or the preview stops matching the file that gets written.
fn apply_ops(image: DynamicImage, ops: &EditOps) -> DynamicImage {
    let image = match ops.rotation.rem_euclid(360) {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    };
    let image = if ops.flip_horizontal { image.fliph() } else { image };
    let image = if ops.flip_vertical { image.flipv() } else { image };

    let image = match &ops.crop {
        Some(crop) if crop.width > 0.0 && crop.height > 0.0 => {
            let (width, height) = (image.width() as f64, image.height() as f64);
            let x = (crop.x * width).round().clamp(0.0, width - 1.0) as u32;
            let y = (crop.y * height).round().clamp(0.0, height - 1.0) as u32;
            let crop_width = (crop.width * width).round().clamp(1.0, width - x as f64) as u32;
            let crop_height = (crop.height * height).round().clamp(1.0, height - y as f64) as u32;
            image.crop_imm(x, y, crop_width, crop_height)
        }
        _ => image,
    };

    adjust_colour(image, ops)
}

/// Writes the image, then moves it into place in one step.
///
/// The rename is what matters. Watched folders are watched: the moment a file
/// appears, the watcher opens it to index and thumbnail it. Encoding a large
/// photo takes seconds, so writing straight to the destination leaves a long
/// window in which the watcher reads a half-written file — which it reported as
/// `failed to decode image: unexpected end of file`, and which could index a
/// truncated photo as if it were the real one.
///
/// A rename within the same directory is atomic: the watcher sees nothing, or it
/// sees the finished file. `ensure_models` already downloads this way.
fn write_image(image: &DynamicImage, target: &Path) -> Result<(), String> {
    // Derived from the real destination — the staging name ends in `.part` and
    // would otherwise be read as an unknown format.
    let format = ImageFormat::from_path(target).unwrap_or(ImageFormat::Jpeg);
    let staging = target.with_extension(format!(
        "{}.part",
        target
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default()
    ));

    let write_to = |path: &Path| -> Result<(), String> {
        if format == ImageFormat::Jpeg {
            // JPEG has no alpha channel, and the encoder needs an explicit quality.
            let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::BufWriter::new(file),
                JPEG_QUALITY,
            );
            return encoder
                .encode_image(&image.to_rgb8())
                .map_err(|e| e.to_string());
        }
        image
            .save_with_format(path, format)
            .map_err(|e| e.to_string())
    };

    write_to(&staging)?;
    if let Err(error) = std::fs::rename(&staging, target) {
        let _ = std::fs::remove_file(&staging);
        return Err(error.to_string());
    }
    Ok(())
}

/// Re-encoding drops the EXIF block, because the imaging crate writes pixels and
/// nothing else. Hive keeps its own copy of what it had already extracted, so
/// Places and the timeline keep working — but the file itself no longer carries
/// it for other programs. That cost is spelled out in the save dialog.
fn preserve_extracted_metadata(
    conn: &Connection,
    media_id: &str,
    taken_at: Option<String>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE media_items SET taken_at = COALESCE(?1, taken_at) WHERE id = ?2",
        params![taken_at, media_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Bakes the adjustments into a real file.
///
/// `mode` is the user's answer to "keep the original?":
/// * `"copy"` — writes `name (edited).ext` beside it and indexes it as a new item.
///   The original keeps its pixels and its EXIF.
/// * `"overwrite"` — replaces the original. Favorites, albums, hidden and archived
///   flags all survive, because the database row keeps its id.
#[tauri::command]
pub fn apply_edits(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    ops: EditOps,
    mode: String,
) -> Result<MediaItem, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let source_path: String = conn
        .query_row(
            "SELECT path FROM media_items WHERE id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    let image = image::open(&source_path).map_err(|e| e.to_string())?;
    let edited = apply_ops(image, &ops);

    save_derived_image(&app, &state, &conn, &media_id, &edited, &mode, "edited")
}

/// Writes a produced image beside — or over — its source, then makes the library
/// agree with what is now on disk.
///
/// Shared by the colour editor and by every AI tool, because "save a copy or
/// replace the original?" has to mean exactly the same thing in all of them.
/// `suffix` is the word placed in the copy's name, e.g. `photo (enlarged).jpg`.
pub fn save_derived_image(
    app: &AppHandle,
    state: &State<'_, AppState>,
    conn: &Connection,
    media_id: &str,
    produced: &DynamicImage,
    mode: &str,
    suffix: &str,
) -> Result<MediaItem, String> {
    let (source_path, filename, folder_id, taken_at): (String, String, String, Option<String>) = conn
        .query_row(
            "SELECT path, filename, folder_id, taken_at FROM media_items WHERE id = ?1",
            params![media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| e.to_string())?;

    let source = PathBuf::from(&source_path);
    let edited = produced;
    let now = chrono::Utc::now().to_rfc3339();

    let target = match mode {
        "overwrite" => source.clone(),
        "copy" => {
            let stem = Path::new(&filename)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| filename.clone());
            let extension = Path::new(&filename)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let parent = source.parent().unwrap_or(Path::new("."));
            unique_destination(parent, &format!("{stem} ({suffix}){extension}"))
        }
        other => return Err(format!("Unknown save mode: {other}")),
    };

    write_image(edited, &target)?;

    // Re-index so size, dimensions and hash match what is now on disk.
    let indexed = indexing::index_file(conn, &folder_id, &target).map_err(|e| e.to_string())?;
    let edited_id = match indexed {
        Some(file) => file.item.id,
        // An overwrite that somehow produced identical bytes leaves the row alone.
        None => conn
            .query_row(
                "SELECT id FROM media_items WHERE path = ?1",
                params![target.to_string_lossy()],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?,
    };

    preserve_extracted_metadata(conn, &edited_id, taken_at)?;
    let _ = thumbnails::generate_for_image(conn, &state.app_data_dir, &edited_id, &target);
    conn.execute(
        "UPDATE media_items SET edited_at = ?1 WHERE id = ?2",
        params![now, edited_id],
    )
    .map_err(|e| e.to_string())?;

    // A copy inherits the description the user wrote for the original.
    if mode == "copy" {
        let carried: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT title, description FROM media_items WHERE id = ?1",
                params![media_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((title, description)) = carried {
            conn.execute(
                "UPDATE media_items SET title = ?1, description = ?2 WHERE id = ?3",
                params![title, description, edited_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    let item = row_to_media_item(conn, &edited_id).map_err(|e| e.to_string())?;
    let _ = tauri::Emitter::emit(app, "media:changed", &folder_id);
    Ok(item)
}

/// Metadata the user types is stored by Hive, never written into the file.
/// The EXIF crate in use can only read, and rewriting a photo just to change its
/// caption would re-encode every pixel for nothing.
#[tauri::command]
pub fn update_media_metadata(
    state: State<'_, AppState>,
    media_id: String,
    title: Option<String>,
    description: Option<String>,
    taken_at_override: Option<String>,
) -> Result<MediaItem, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let blank_to_none = |value: Option<String>| {
        value
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    };

    conn.execute(
        "UPDATE media_items SET title = ?1, description = ?2, taken_at_override = ?3 WHERE id = ?4",
        params![
            blank_to_none(title),
            blank_to_none(description),
            blank_to_none(taken_at_override),
            media_id
        ],
    )
    .map_err(|e| e.to_string())?;

    row_to_media_item(&conn, &media_id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CropRect;

    fn ops() -> EditOps {
        EditOps {
            rotation: 0,
            flip_horizontal: false,
            flip_vertical: false,
            crop: None,
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            grayscale: 0.0,
            sepia: 0.0,
            temperature: 0.0,
        }
    }

    fn grey_square() -> DynamicImage {
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            40,
            20,
            image::Rgba([128, 128, 128, 255]),
        ))
    }

    #[test]
    fn neutral_ops_leave_pixels_alone() {
        let result = apply_ops(grey_square(), &ops());
        assert_eq!(result.to_rgba8().get_pixel(0, 0)[0], 128);
        assert_eq!((result.width(), result.height()), (40, 20));
    }

    #[test]
    fn rotation_swaps_the_sides() {
        let rotated = apply_ops(grey_square(), &EditOps { rotation: 90, ..ops() });
        assert_eq!((rotated.width(), rotated.height()), (20, 40));
    }

    #[test]
    fn crop_is_measured_after_rotation() {
        // 40x20 rotated to 20x40, then the middle half of the height taken.
        let cropped = apply_ops(
            grey_square(),
            &EditOps {
                rotation: 90,
                crop: Some(CropRect { x: 0.0, y: 0.25, width: 1.0, height: 0.5 }),
                ..ops()
            },
        );
        assert_eq!((cropped.width(), cropped.height()), (20, 20));
    }

    #[test]
    fn brightness_scales_and_clamps() {
        let brighter = apply_ops(grey_square(), &EditOps { brightness: 1.5, ..ops() });
        assert_eq!(brighter.to_rgba8().get_pixel(0, 0)[0], 192);

        let blown = apply_ops(grey_square(), &EditOps { brightness: 10.0, ..ops() });
        assert_eq!(blown.to_rgba8().get_pixel(0, 0)[0], 255, "must clamp, not wrap");
    }

    #[test]
    fn desaturating_grey_changes_nothing() {
        let flat = apply_ops(grey_square(), &EditOps { saturation: 0.0, ..ops() });
        assert_eq!(flat.to_rgba8().get_pixel(0, 0)[0], 128);
    }

    #[test]
    fn saturation_pushes_colour_away_from_luma() {
        let red = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([200, 100, 100, 255]),
        ));
        let flat = apply_ops(red, &EditOps { saturation: 0.0, ..ops() });
        let pixel = flat.to_rgba8().get_pixel(0, 0).0;
        assert_eq!(pixel[0], pixel[1], "fully desaturated pixels are grey");
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn full_grayscale_flattens_every_channel() {
        let red = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([220, 40, 60, 255]),
        ));
        let flat = apply_ops(red, &EditOps { grayscale: 1.0, ..ops() });
        let pixel = flat.to_rgba8().get_pixel(0, 0).0;
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn sepia_warms_a_grey_towards_orange() {
        let grey = grey_square();
        let toned = apply_ops(grey, &EditOps { sepia: 1.0, ..ops() });
        let pixel = toned.to_rgba8().get_pixel(0, 0).0;
        assert!(pixel[0] > pixel[1], "red should lead");
        assert!(pixel[1] > pixel[2], "blue should trail");
    }

    #[test]
    fn temperature_moves_red_and_blue_in_opposite_directions() {
        let warm = apply_ops(grey_square(), &EditOps { temperature: 1.0, ..ops() });
        let warm_pixel = warm.to_rgba8().get_pixel(0, 0).0;
        assert!(warm_pixel[0] > 128 && warm_pixel[2] < 128);

        let cool = apply_ops(grey_square(), &EditOps { temperature: -1.0, ..ops() });
        let cool_pixel = cool.to_rgba8().get_pixel(0, 0).0;
        assert!(cool_pixel[0] < 128 && cool_pixel[2] > 128);
    }

    #[test]
    fn the_neutral_check_covers_every_new_field() {
        assert!(is_neutral(&ops()));
        assert!(!is_neutral(&EditOps { grayscale: 0.5, ..ops() }));
        assert!(!is_neutral(&EditOps { sepia: 0.5, ..ops() }));
        assert!(!is_neutral(&EditOps { temperature: 0.5, ..ops() }));
    }

    #[test]
    fn alpha_survives_every_adjustment() {
        let transparent = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([10, 20, 30, 77]),
        ));
        let result = apply_ops(transparent, &EditOps { brightness: 2.0, contrast: 1.4, ..ops() });
        assert_eq!(result.to_rgba8().get_pixel(0, 0)[3], 77);
    }
}
