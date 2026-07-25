use crate::models::{BatchReport, RenamePreview};
use crate::state::AppState;
use image::codecs::jpeg::JpegEncoder;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

/// Characters Windows refuses in a filename. Anything else the user types is
/// kept as-is.
const ILLEGAL_FILENAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|c| !ILLEGAL_FILENAME_CHARS.contains(c))
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string()
}

/// Expands the rename pattern for one item.
/// `{name}` original stem · `{n}` sequence number, 3 digits · `{date}` capture day.
/// The extension is never part of the pattern — it is always carried over from
/// the original file, so a pattern can not produce an unopenable name.
fn expand_pattern(pattern: &str, stem: &str, index: i64, taken_at: Option<&str>) -> String {
    let date = taken_at
        .and_then(|value| value.get(0..10))
        .unwrap_or("undated")
        .to_string();

    let expanded = pattern
        .replace("{name}", stem)
        .replace("{n}", &format!("{index:03}"))
        .replace("{date}", &date);

    let cleaned = sanitize(&expanded);
    if cleaned.is_empty() {
        format!("{stem}-{index:03}")
    } else {
        cleaned
    }
}

fn read_rows(
    conn: &Connection,
    media_ids: &[String],
) -> Result<Vec<(String, String, String, Option<String>)>, String> {
    let mut rows = Vec::with_capacity(media_ids.len());
    for media_id in media_ids {
        let row = conn.query_row(
            "SELECT id, path, filename, taken_at FROM media_items WHERE id = ?1",
            params![media_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        );
        if let Ok(row) = row {
            rows.push(row);
        }
    }
    Ok(rows)
}

fn build_previews(
    rows: &[(String, String, String, Option<String>)],
    pattern: &str,
    start_index: i64,
) -> Vec<RenamePreview> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut previews = Vec::with_capacity(rows.len());

    for (offset, (media_id, path, filename, taken_at)) in rows.iter().enumerate() {
        let source = Path::new(path);
        let stem = source.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let extension = source
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        let new_stem = expand_pattern(pattern, &stem, start_index + offset as i64, taken_at.as_deref());
        let new_name = format!("{new_stem}{extension}");
        let target = source.with_file_name(&new_name);

        // Renaming a file to the name it already has is a no-op, not a clash.
        let unchanged = new_name == *filename;
        let conflict = !unchanged
            && (target.exists() || !taken.insert(target.to_string_lossy().to_string()));

        previews.push(RenamePreview {
            media_id: media_id.clone(),
            from: filename.clone(),
            to: new_name,
            conflict,
        });
    }

    previews
}

/// Shows what a rename would produce, without touching anything.
#[tauri::command]
pub fn preview_batch_rename(
    state: State<'_, AppState>,
    media_ids: Vec<String>,
    pattern: String,
    start_index: Option<i64>,
) -> Result<Vec<RenamePreview>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = read_rows(&conn, &media_ids)?;
    Ok(build_previews(&rows, &pattern, start_index.unwrap_or(1)))
}

/// Renames the files on disk and follows them in the index.
///
/// The database row is updated *before* the file moves. That order matters: the
/// folder watcher reacts to the new file appearing, and by then the index already
/// knows that path, so it recognises the file instead of adding a second copy.
/// If the move then fails, the row is put back.
#[tauri::command]
pub fn batch_rename(
    app: AppHandle,
    state: State<'_, AppState>,
    media_ids: Vec<String>,
    pattern: String,
    start_index: Option<i64>,
) -> Result<BatchReport, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = read_rows(&conn, &media_ids)?;
    let previews = build_previews(&rows, &pattern, start_index.unwrap_or(1));

    let mut report = BatchReport {
        processed: 0,
        skipped: 0,
        failed: 0,
        bytes_before: 0,
        bytes_after: 0,
        destination: None,
        first_error: None,
    };

    for (preview, (_, old_path, old_name, _)) in previews.iter().zip(rows.iter()) {
        if preview.conflict || preview.to == *old_name {
            report.skipped += 1;
            continue;
        }

        let source = PathBuf::from(old_path);
        let target = source.with_file_name(&preview.to);
        let target_string = target.to_string_lossy().to_string();

        if conn
            .execute(
                "UPDATE media_items SET path = ?1, filename = ?2 WHERE id = ?3",
                params![target_string, preview.to, preview.media_id],
            )
            .is_err()
        {
            report.failed += 1;
            continue;
        }

        match std::fs::rename(&source, &target) {
            Ok(()) => {
                let _ = conn.execute(
                    "UPDATE media_fts SET filename = ?1 WHERE media_id = ?2",
                    params![preview.to, preview.media_id],
                );
                report.processed += 1;
            }
            Err(error) => {
                let _ = conn.execute(
                    "UPDATE media_items SET path = ?1, filename = ?2 WHERE id = ?3",
                    params![old_path, old_name, preview.media_id],
                );
                report.failed += 1;
                report.first_error.get_or_insert_with(|| error.to_string());
            }
        }
    }

    let _ = tauri::Emitter::emit(&app, "media:changed", "batch-rename");
    Ok(report)
}

/// Loads an image, optionally shrinks it so neither side exceeds `max_dimension`,
/// and writes a JPEG at the requested quality into `destination`.
///
/// Always writes a new file next to nothing else — the original is never opened
/// for writing, so a bad quality setting costs you a re-run, never a photo.
#[tauri::command]
pub fn compress_images(
    state: State<'_, AppState>,
    media_ids: Vec<String>,
    quality: u8,
    max_dimension: Option<u32>,
    destination: String,
) -> Result<BatchReport, String> {
    let destination_dir = PathBuf::from(&destination);
    if !destination_dir.is_dir() {
        return Err("The destination is not a folder".into());
    }
    let quality = quality.clamp(1, 100);

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = read_rows(&conn, &media_ids)?;

    let mut report = BatchReport {
        processed: 0,
        skipped: 0,
        failed: 0,
        bytes_before: 0,
        bytes_after: 0,
        destination: Some(destination.clone()),
        first_error: None,
    };

    for (_, path, filename, _) in rows {
        let source = Path::new(&path);
        let Ok(metadata) = std::fs::metadata(source) else {
            report.skipped += 1;
            continue;
        };

        let image = match image::open(source) {
            Ok(image) => image,
            Err(_) => {
                report.skipped += 1;
                continue;
            }
        };

        let image = match max_dimension {
            Some(max) if max > 0 && (image.width() > max || image.height() > max) => {
                image.thumbnail(max, max)
            }
            _ => image,
        };

        let stem = Path::new(&filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| filename.clone());
        let target = crate::commands::utilities::unique_destination(
            &destination_dir,
            &format!("{stem}.jpg"),
        );

        let write = std::fs::File::create(&target).map_err(|e| e.to_string()).and_then(|file| {
            let mut encoder = JpegEncoder::new_with_quality(std::io::BufWriter::new(file), quality);
            encoder
                .encode_image(&image.to_rgb8())
                .map_err(|e| e.to_string())
        });

        match write {
            Ok(()) => {
                report.processed += 1;
                report.bytes_before += metadata.len() as i64;
                report.bytes_after += std::fs::metadata(&target).map(|m| m.len() as i64).unwrap_or(0);
            }
            Err(error) => {
                report.failed += 1;
                report.first_error.get_or_insert(error);
            }
        }
    }

    Ok(report)
}

/// Rewrites images into another container (jpg, png or webp) inside `destination`.
/// Like compression, this only ever creates new files.
#[tauri::command]
pub fn convert_images(
    state: State<'_, AppState>,
    media_ids: Vec<String>,
    format: String,
    destination: String,
) -> Result<BatchReport, String> {
    let destination_dir = PathBuf::from(&destination);
    if !destination_dir.is_dir() {
        return Err("The destination is not a folder".into());
    }

    let (image_format, extension) = match format.to_lowercase().as_str() {
        "png" => (image::ImageFormat::Png, "png"),
        "webp" => (image::ImageFormat::WebP, "webp"),
        "jpg" | "jpeg" => (image::ImageFormat::Jpeg, "jpg"),
        other => return Err(format!("Unsupported target format: {other}")),
    };

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = read_rows(&conn, &media_ids)?;

    let mut report = BatchReport {
        processed: 0,
        skipped: 0,
        failed: 0,
        bytes_before: 0,
        bytes_after: 0,
        destination: Some(destination.clone()),
        first_error: None,
    };

    for (_, path, filename, _) in rows {
        let source = Path::new(&path);
        let Ok(metadata) = std::fs::metadata(source) else {
            report.skipped += 1;
            continue;
        };

        let image = match image::open(source) {
            Ok(image) => image,
            Err(_) => {
                report.skipped += 1;
                continue;
            }
        };

        let stem = Path::new(&filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| filename.clone());
        let target = crate::commands::utilities::unique_destination(
            &destination_dir,
            &format!("{stem}.{extension}"),
        );

        // JPEG has no alpha channel, so a transparent PNG must be flattened first.
        let encoded = if image_format == image::ImageFormat::Jpeg {
            image::DynamicImage::ImageRgb8(image.to_rgb8()).save_with_format(&target, image_format)
        } else {
            image.save_with_format(&target, image_format)
        };

        match encoded {
            Ok(()) => {
                report.processed += 1;
                report.bytes_before += metadata.len() as i64;
                report.bytes_after += std::fs::metadata(&target).map(|m| m.len() as i64).unwrap_or(0);
            }
            Err(error) => {
                report.failed += 1;
                report.first_error.get_or_insert_with(|| error.to_string());
            }
        }
    }

    Ok(report)
}

/// Re-encodes videos into `destination` with H.264, using the same system
/// `ffmpeg` binary that already produces video thumbnails.
///
/// `quality` is a CRF value: lower is better-looking and larger, higher is
/// smaller and softer. 18 is visually lossless, 28 is noticeably compressed.
/// Like every other batch operation, this only ever writes new files.
#[tauri::command]
pub fn compress_videos(
    state: State<'_, AppState>,
    media_ids: Vec<String>,
    quality: u8,
    max_height: Option<u32>,
    destination: String,
) -> Result<BatchReport, String> {
    if !crate::thumbnails::ffmpeg_available() {
        return Err(
            "ffmpeg is not on your PATH. Install it to compress videos — Hive uses the same \
             binary it already needs for video thumbnails."
                .into(),
        );
    }

    let destination_dir = PathBuf::from(&destination);
    if !destination_dir.is_dir() {
        return Err("The destination is not a folder".into());
    }
    let crf = quality.clamp(16, 40).to_string();

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = read_rows(&conn, &media_ids)?;

    let mut report = BatchReport {
        processed: 0,
        skipped: 0,
        failed: 0,
        bytes_before: 0,
        bytes_after: 0,
        destination: Some(destination.clone()),
        first_error: None,
    };

    for (_, path, filename, _) in rows {
        let source = Path::new(&path);
        let Ok(metadata) = std::fs::metadata(source) else {
            report.skipped += 1;
            continue;
        };

        let stem = Path::new(&filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| filename.clone());
        let target = crate::commands::utilities::unique_destination(
            &destination_dir,
            &format!("{stem}.mp4"),
        );

        let mut args: Vec<String> = vec![
            "-y".into(),
            "-i".into(),
            path.clone(),
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf.clone(),
            "-preset".into(),
            "medium".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "128k".into(),
        ];
        if let Some(height) = max_height.filter(|h| *h > 0) {
            // -2 keeps the width even, which H.264 requires.
            args.push("-vf".into());
            args.push(format!("scale=-2:min({height}\\,ih)"));
        }
        args.push(target.to_string_lossy().to_string());

        match std::process::Command::new("ffmpeg").args(&args).output() {
            Ok(output) if output.status.success() => {
                report.processed += 1;
                report.bytes_before += metadata.len() as i64;
                report.bytes_after +=
                    std::fs::metadata(&target).map(|m| m.len() as i64).unwrap_or(0);
            }
            Ok(output) => {
                report.failed += 1;
                report.first_error.get_or_insert_with(|| {
                    String::from_utf8_lossy(&output.stderr)
                        .lines()
                        .last()
                        .unwrap_or("ffmpeg failed")
                        .to_string()
                });
                let _ = std::fs::remove_file(&target);
            }
            Err(error) => {
                report.failed += 1;
                report.first_error.get_or_insert_with(|| error.to_string());
            }
        }
    }

    Ok(report)
}

/// Whether video compression is available on this machine.
#[tauri::command]
pub fn video_tools_available() -> bool {
    crate::thumbnails::ffmpeg_available()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_expands_every_token() {
        let name = expand_pattern("{date}-{name}-{n}", "IMG_2847", 7, Some("2024-06-15T10:00:00Z"));
        assert_eq!(name, "2024-06-15-IMG_2847-007");
    }

    #[test]
    fn pattern_drops_illegal_characters() {
        assert_eq!(expand_pattern("hol/i:day", "x", 1, None), "holiday");
    }

    #[test]
    fn pattern_never_yields_an_empty_name() {
        assert_eq!(expand_pattern("///", "IMG_1", 4, None), "IMG_1-004");
    }

    #[test]
    fn undated_photos_get_a_placeholder_date() {
        assert_eq!(expand_pattern("{date}", "x", 1, None), "undated");
    }

    #[test]
    fn previews_flag_duplicate_targets_inside_the_batch() {
        let rows = vec![
            ("a".into(), "C:\\p\\one.jpg".into(), "one.jpg".into(), None),
            ("b".into(), "C:\\p\\two.jpg".into(), "two.jpg".into(), None),
        ];
        // A constant pattern maps both files onto the same name.
        let previews = build_previews(&rows, "same", 1);
        assert!(!previews[0].conflict);
        assert!(previews[1].conflict, "the second collision must be flagged");
    }
}
