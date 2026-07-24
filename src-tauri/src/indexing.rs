use crate::models::{ExifData, MediaItem};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::UNIX_EPOCH;

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif"];
const VIDEO_EXTS: &[&str] = &["mp4", "mov", "avi", "mkv", "webm"];

pub fn is_supported_media(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if IMAGE_EXTS.contains(&ext.as_str()) {
        Some("image")
    } else if VIDEO_EXTS.contains(&ext.as_str()) {
        Some("video")
    } else {
        None
    }
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_exif(path: &Path) -> Option<ExifData> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);
    let exif = exif::Reader::new()
        .read_from_container(&mut bufreader)
        .ok()?;

    let get_str = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string())
    };
    let get_f64 = |tag: exif::Tag| -> Option<f64> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| match &f.value {
                exif::Value::Rational(v) if !v.is_empty() => Some(v[0].to_f64()),
                exif::Value::SRational(v) if !v.is_empty() => Some(v[0].to_f64()),
                _ => f.value.get_uint(0).map(|u| u as f64),
            })
    };
    let get_i64 = |tag: exif::Tag| -> Option<i64> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| f.value.get_uint(0))
            .map(|u| u as i64)
    };

    Some(ExifData {
        camera_make: get_str(exif::Tag::Make),
        camera_model: get_str(exif::Tag::Model),
        lens: get_str(exif::Tag::LensModel),
        iso: get_i64(exif::Tag::PhotographicSensitivity),
        f_number: get_f64(exif::Tag::FNumber),
        exposure_time: get_str(exif::Tag::ExposureTime),
        focal_length: get_f64(exif::Tag::FocalLength),
        gps_lat: gps_coord(&exif, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef),
        gps_lon: gps_coord(&exif, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef),
    })
}

fn gps_coord(exif: &exif::Exif, tag: exif::Tag, ref_tag: exif::Tag) -> Option<f64> {
    let field = exif.get_field(tag, exif::In::PRIMARY)?;
    let vals = match &field.value {
        exif::Value::Rational(v) if v.len() == 3 => v,
        _ => return None,
    };
    let deg = vals[0].to_f64() + vals[1].to_f64() / 60.0 + vals[2].to_f64() / 3600.0;
    let sign = exif
        .get_field(ref_tag, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string())
        .map(|s| if s.starts_with('S') || s.starts_with('W') { -1.0 } else { 1.0 })
        .unwrap_or(1.0);
    Some(deg * sign)
}

fn exif_taken_at(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bufreader = BufReader::new(file);
    let exif = exif::Reader::new()
        .read_from_container(&mut bufreader)
        .ok()?;
    let field = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
    let raw = field.display_value().to_string();
    // EXIF format: "YYYY:MM:DD HH:MM:SS" -> normalize to ISO-ish
    let normalized = raw.replacen(':', "-", 2);
    Some(normalized)
}

pub struct IndexedFile {
    pub item: MediaItem,
}

/// Index a single file: hash, extract metadata, insert/update the DB row.
/// Returns None if the file is unsupported or already indexed with the same hash.
pub fn index_file(
    conn: &Connection,
    folder_id: &str,
    path: &Path,
) -> anyhow::Result<Option<IndexedFile>> {
    let media_type = match is_supported_media(path) {
        Some(t) => t,
        None => return Ok(None),
    };

    let metadata = std::fs::metadata(path)?;
    let hash = hash_file(path)?;

    let existing: Option<String> = conn
        .query_row(
            "SELECT hash FROM media_items WHERE path = ?1",
            params![path.to_string_lossy()],
            |r| r.get(0),
        )
        .optional()?;
    if existing.as_deref() == Some(hash.as_str()) {
        return Ok(None);
    }

    let (width, height) = if media_type == "image" {
        image::image_dimensions(path)
            .map(|(w, h)| (Some(w as i64), Some(h as i64)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    let exif = if media_type == "image" { read_exif(path) } else { None };
    let taken_at = if media_type == "image" { exif_taken_at(path) } else { None };

    let now: DateTime<Utc> = Utc::now();
    let modified_at: DateTime<Utc> = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
        .unwrap_or(now);
    let created_at: DateTime<Utc> = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
        .unwrap_or(now);

    let id = uuid::Uuid::new_v4().to_string();
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    conn.execute(
        "INSERT INTO media_items
            (id, folder_id, path, filename, hash, size, width, height, duration_ms,
             mime_type, media_type, taken_at, created_at, modified_at, indexed_at,
             is_favorite, is_trashed)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10,?11,?12,?13,?14,0,0)
         ON CONFLICT(path) DO UPDATE SET
            hash=excluded.hash, size=excluded.size, width=excluded.width,
            height=excluded.height, mime_type=excluded.mime_type,
            media_type=excluded.media_type, taken_at=excluded.taken_at,
            modified_at=excluded.modified_at, indexed_at=excluded.indexed_at",
        params![
            id,
            folder_id,
            path.to_string_lossy(),
            filename,
            hash,
            metadata.len() as i64,
            width,
            height,
            mime_type,
            media_type,
            taken_at,
            created_at.to_rfc3339(),
            modified_at.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    let media_id: String = conn.query_row(
        "SELECT id FROM media_items WHERE path = ?1",
        params![path.to_string_lossy()],
        |r| r.get(0),
    )?;

    if let Some(exif) = &exif {
        conn.execute(
            "INSERT INTO exif_data
                (media_id, camera_make, camera_model, lens, iso, f_number,
                 exposure_time, focal_length, gps_lat, gps_lon)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(media_id) DO UPDATE SET
                camera_make=excluded.camera_make, camera_model=excluded.camera_model,
                lens=excluded.lens, iso=excluded.iso, f_number=excluded.f_number,
                exposure_time=excluded.exposure_time, focal_length=excluded.focal_length,
                gps_lat=excluded.gps_lat, gps_lon=excluded.gps_lon",
            params![
                media_id,
                exif.camera_make,
                exif.camera_model,
                exif.lens,
                exif.iso,
                exif.f_number,
                exif.exposure_time,
                exif.focal_length,
                exif.gps_lat,
                exif.gps_lon,
            ],
        )?;
    }

    upsert_fts(conn, &media_id, &filename, exif.as_ref())?;

    Ok(Some(IndexedFile {
        item: MediaItem {
            id: media_id,
            folder_id: folder_id.to_string(),
            path: path.to_string_lossy().to_string(),
            filename,
            hash,
            size: metadata.len() as i64,
            width,
            height,
            duration_ms: None,
            mime_type,
            media_type: media_type.to_string(),
            taken_at,
            created_at: created_at.to_rfc3339(),
            modified_at: modified_at.to_rfc3339(),
            indexed_at: now.to_rfc3339(),
            is_favorite: false,
            is_trashed: false,
            exif,
            thumbnail_path: None,
        },
    }))
}

fn upsert_fts(
    conn: &Connection,
    media_id: &str,
    filename: &str,
    exif: Option<&ExifData>,
) -> rusqlite::Result<()> {
    let existing_ocr: String = conn
        .query_row(
            "SELECT ocr_text FROM media_fts WHERE media_id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .unwrap_or_default();
    conn.execute(
        "DELETE FROM media_fts WHERE media_id = ?1",
        params![media_id],
    )?;
    conn.execute(
        "INSERT INTO media_fts (media_id, filename, camera_make, camera_model, ocr_text)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            media_id,
            filename,
            exif.and_then(|e| e.camera_make.clone()),
            exif.and_then(|e| e.camera_model.clone()),
            existing_ocr,
        ],
    )?;
    Ok(())
}

/// Replace the OCR text indexed for a media item. Wired up by the OCR pipeline (Phase 5).
#[allow(dead_code)]
pub fn update_fts_ocr_text(conn: &Connection, media_id: &str, ocr_text: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE media_fts SET ocr_text = ?2 WHERE media_id = ?1",
        params![media_id, ocr_text],
    )?;
    Ok(())
}

pub fn walk_folder(root: &Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .filter(|p| is_supported_media(p).is_some())
        .collect()
}
