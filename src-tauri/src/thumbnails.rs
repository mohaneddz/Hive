use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const SIZE_SM: (&str, u32) = ("sm", 240);
pub const SIZE_MD: (&str, u32) = ("md", 800);

pub fn thumbnail_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("thumbnails")
}

fn save_thumbnails(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    img: &image::DynamicImage,
) -> anyhow::Result<()> {
    let out_dir = thumbnail_dir(app_data_dir);
    std::fs::create_dir_all(&out_dir)?;

    for (label, max_dim) in [SIZE_SM, SIZE_MD] {
        let thumb = img.thumbnail(max_dim, max_dim);
        let file_name = format!("{media_id}_{label}.jpg");
        let out_path = out_dir.join(&file_name);
        thumb.to_rgb8().save_with_format(&out_path, image::ImageFormat::Jpeg)?;

        conn.execute(
            "INSERT INTO thumbnails (media_id, size, path) VALUES (?1, ?2, ?3)
             ON CONFLICT(media_id, size) DO UPDATE SET path = excluded.path",
            params![media_id, label, out_path.to_string_lossy()],
        )?;
    }

    Ok(())
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
        Err(e) => {
            eprintln!("[thumbnails] failed to decode image {source_path:?}: {e}");
            return Ok(false);
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

    let frame_path = std::env::temp_dir().join(format!("hive_frame_{media_id}.jpg"));

    // Grab a frame 1s in (skips solid-color opening frames some videos have) with a fallback to
    // the very first frame for clips shorter than that.
    let attempts: [&[&str]; 2] = [
        &["-y", "-ss", "00:00:01", "-i", "IN", "-frames:v", "1", "-q:v", "3", "OUT"],
        &["-y", "-i", "IN", "-frames:v", "1", "-q:v", "3", "OUT"],
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
