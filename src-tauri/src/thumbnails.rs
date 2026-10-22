use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const SIZE_SM: (&str, u32) = ("sm", 240);
pub const SIZE_MD: (&str, u32) = ("md", 800);

pub fn thumbnail_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("thumbnails")
}

/// A browser-readable preview for image formats WebView2 cannot reliably decode.
/// The hash in the name makes an edited original invalidate its old preview.
pub fn viewer_preview(app_data_dir: &Path, media_id: &str, hash: &str, source: &Path) -> anyhow::Result<PathBuf> {
    let dir = thumbnail_dir(app_data_dir);
    std::fs::create_dir_all(&dir)?;
    let safe_hash: String = hash.chars().filter(|c| c.is_ascii_hexdigit()).take(16).collect();
    let target = dir.join(format!("{media_id}_{safe_hash}_preview.jpg"));
    if target.is_file() {
        return Ok(target);
    }

    let temporary = dir.join(format!("{media_id}_{}_preview.jpg", uuid::Uuid::new_v4()));
    let generated = if ffmpeg_available() {
        std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
            .arg(source)
            .args(["-frames:v", "1", "-vf", "scale='min(4096,iw)':'min(4096,ih)':force_original_aspect_ratio=decrease", "-q:v", "2"])
            .arg(&temporary)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    } else {
        false
    };
    if !generated {
        // TIFF and AVIF can be handled by the Rust decoder even without ffmpeg.
        let fallback = (|| -> anyhow::Result<()> {
            let image = image::open(source)?;
            image.thumbnail(4096, 4096).save_with_format(&temporary, image::ImageFormat::Jpeg)?;
            Ok(())
        })();
        if let Err(error) = fallback {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    }
    if let Err(error) = std::fs::rename(&temporary, &target) {
        let _ = std::fs::remove_file(&temporary);
        if !target.is_file() {
            return Err(error.into());
        }
    }
    Ok(target)
}

fn save_thumbnails(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    img: &image::DynamicImage,
) -> anyhow::Result<()> {
    let out_dir = thumbnail_dir(app_data_dir);
    std::fs::create_dir_all(&out_dir)?;

    let hash: String = conn.query_row(
        "SELECT hash FROM media_items WHERE id = ?1",
        params![media_id],
        |row| row.get(0),
    )?;
    let short_hash: String = hash.chars().filter(|ch| ch.is_ascii_hexdigit()).take(16).collect();

    let medium = img.thumbnail(SIZE_MD.1, SIZE_MD.1);
    let small = medium.thumbnail(SIZE_SM.1, SIZE_SM.1);
    for (label, thumb) in [(SIZE_SM.0, &small), (SIZE_MD.0, &medium)] {
        let file_name = format!("{media_id}_{short_hash}_{label}.jpg");
        let out_path = out_dir.join(&file_name);
        thumb.to_rgb8().save_with_format(&out_path, image::ImageFormat::Jpeg)?;

        let previous: Option<String> = conn.query_row(
            "SELECT path FROM thumbnails WHERE media_id = ?1 AND size = ?2",
            params![media_id, label],
            |row| row.get(0),
        ).ok();

        conn.execute(
            "INSERT INTO thumbnails (media_id, size, path) VALUES (?1, ?2, ?3)
             ON CONFLICT(media_id, size) DO UPDATE SET path = excluded.path",
            params![media_id, label, out_path.to_string_lossy()],
        )?;
        if let Some(old) = previous {
            let old = PathBuf::from(old);
            if old != out_path && old.parent() == Some(out_dir.as_path()) {
                let _ = std::fs::remove_file(old);
            }
        }
    }

    Ok(())
}

pub fn generate_from_decoded_image(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    image: &image::DynamicImage,
) -> anyhow::Result<()> {
    save_thumbnails(conn, app_data_dir, media_id, image)
}

/// Generate sm/md thumbnails for an image media item and record them in the DB.
pub fn generate_for_image(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    source_path: &Path,
) -> anyhow::Result<bool> {
    let img = match image::open(source_path) {
        Ok(img) => img,
        Err(native_error) => {
            // HEIC/HEIF and camera RAW codecs are platform-dependent. When the
            // Rust image stack cannot decode one, use the same optional ffmpeg
            // dependency already supported for video thumbnails.
            if !ffmpeg_available() {
                eprintln!("[thumbnails] failed to decode image {source_path:?}: {native_error}");
                return Ok(false);
            }
            let frame_path = std::env::temp_dir().join(format!("hive_image_{}.jpg", uuid::Uuid::new_v4()));
            let output = std::process::Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
                .arg(source_path)
                .args(["-frames:v", "1", "-vf", "scale='min(800,iw)':'min(800,ih)':force_original_aspect_ratio=decrease", "-q:v", "3"])
                .arg(&frame_path)
                .output();
            let decoded = match output {
                Ok(result) if result.status.success() && frame_path.is_file() => image::open(&frame_path),
                Ok(result) => {
                    eprintln!("[thumbnails] ffmpeg failed for {source_path:?}: {}", String::from_utf8_lossy(&result.stderr));
                    let _ = std::fs::remove_file(&frame_path);
                    return Ok(false);
                }
                Err(error) => {
                    eprintln!("[thumbnails] could not run ffmpeg for {source_path:?}: {error}");
                    return Ok(false);
                }
            };
            let _ = std::fs::remove_file(&frame_path);
            decoded?
        }
    };
    save_thumbnails(conn, app_data_dir, media_id, &img)?;
    Ok(true)
}

pub fn ffmpeg_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Generate sm/md thumbnails for a video by extracting a single frame via the system `ffmpeg`
/// binary. No-op (returns `Ok(false)`) if ffmpeg isn't on PATH — there's no bundled decoder,
/// so video thumbnails are best-effort depending on what's installed on the machine.
pub fn generate_for_video(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    source_path: &Path,
) -> anyhow::Result<bool> {
    if !ffmpeg_available() {
        return Ok(false);
    }

    let frame_path = std::env::temp_dir().join(format!("hive_frame_{}.jpg", uuid::Uuid::new_v4()));

    // Grab a frame 1s in (skips solid-color opening frames some videos have) with a fallback to
    // the very first frame for clips shorter than that.
    let attempts: [&[&str]; 2] = [
        &["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-ss", "00:00:01", "-i", "IN", "-frames:v", "1", "-vf", "scale='min(800,iw)':'min(800,ih)':force_original_aspect_ratio=decrease", "-q:v", "3", "OUT"],
        &["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i", "IN", "-frames:v", "1", "-vf", "scale='min(800,iw)':'min(800,ih)':force_original_aspect_ratio=decrease", "-q:v", "3", "OUT"],
    ];

    let mut extracted = false;
    for args in attempts {
        let resolved: Vec<String> = args
            .iter()
            .map(|a| match *a {
                "IN" => source_path.to_string_lossy().to_string(),
                "OUT" => frame_path.to_string_lossy().to_string(),
                other => other.to_string(),
            })
            .collect();

        let output = std::process::Command::new("ffmpeg").args(&resolved).output();
        match output {
            Ok(o) if o.status.success() && frame_path.is_file() => {
                extracted = true;
                break;
            }
            Ok(o) => {
                eprintln!(
                    "[thumbnails] ffmpeg frame extraction attempt failed for {source_path:?}: {}",
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            Err(e) => {
                eprintln!("[thumbnails] failed to run ffmpeg for {source_path:?}: {e}");
                break;
            }
        }
    }

    if !extracted {
        return Ok(false);
    }

    let result = (|| -> anyhow::Result<bool> {
        let img = image::open(&frame_path)?;
        save_thumbnails(conn, app_data_dir, media_id, &img)?;
        Ok(true)
    })();

    let _ = std::fs::remove_file(&frame_path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn preview_converts_tiff_once_and_bounds_dimensions() {
        let dir = std::env::temp_dir().join(format!("hive_preview_fixture_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("image.tiff");
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(320, 240, image::Rgb([80, 120, 160])));
        image.save_with_format(&source, image::ImageFormat::Tiff).unwrap();
        let preview = viewer_preview(&dir, "fixture", "0123456789abcdef", &source).unwrap();
        assert_eq!(image::open(&preview).unwrap().dimensions(), (320, 240));
        assert_eq!(viewer_preview(&dir, "fixture", "0123456789abcdef", &source).unwrap(), preview);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Ignored by default since it shells out to ffmpeg and needs a writable rusqlite DB —
    /// run explicitly with `cargo test -- --ignored thumbnails::tests`.
    #[test]
    #[ignore]
    fn extracts_a_thumbnail_from_a_real_video() {
        assert!(ffmpeg_available(), "ffmpeg must be on PATH for this test");

        let dir = std::env::temp_dir().join(format!("hive_video_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let video_path = dir.join("clip.mp4");

        // Synthesize a 2s test-pattern video with ffmpeg's built-in source filter — no
        // external test asset needed.
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=320x240:rate=10",
                video_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(status.status.success(), "failed to synthesize test video");

        let db_path = dir.join("test.db");
        let conn = crate::db::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO folders (id, path, name, is_watched, added_at) VALUES ('f1', ?1, 'test', 1, '2024-01-01')",
            params![dir.to_string_lossy()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO media_items (id, folder_id, path, filename, hash, size, mime_type, media_type,
                created_at, modified_at, indexed_at, is_favorite, is_trashed)
             VALUES ('m1', 'f1', ?1, 'clip.mp4', 'x', 1, 'video/mp4', 'video', '2024-01-01', '2024-01-01', '2024-01-01', 0, 0)",
            params![video_path.to_string_lossy()],
        )
        .unwrap();

        let generated = generate_for_video(&conn, &dir, "m1", &video_path).unwrap();
        assert!(generated, "expected a thumbnail to be generated");

        let thumb_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM thumbnails WHERE media_id = 'm1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(thumb_count, 2, "expected sm + md thumbnails");

        let sm_path: String = conn
            .query_row(
                "SELECT path FROM thumbnails WHERE media_id = 'm1' AND size = 'sm'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let dims = image::image_dimensions(&sm_path).unwrap();
        assert!(dims.0 > 0 && dims.1 > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Dispatches to the image or video thumbnail path based on `media_type`.
pub fn generate(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    source_path: &Path,
    media_type: &str,
) -> anyhow::Result<bool> {
    match media_type {
        "video" => generate_for_video(conn, app_data_dir, media_id, source_path),
        _ => generate_for_image(conn, app_data_dir, media_id, source_path),
    }
}
