use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub const SIZE_SM: (&str, u32) = ("sm", 240);
pub const SIZE_MD: (&str, u32) = ("md", 800);

pub fn thumbnail_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("thumbnails")
}

/// Generate sm/md thumbnails for an image media item and record them in the DB.
/// No-op (returns Ok(false)) for non-image media types.
pub fn generate_for_image(
    conn: &Connection,
    app_data_dir: &Path,
    media_id: &str,
    source_path: &Path,
) -> anyhow::Result<bool> {
    let img = match image::open(source_path) {
        Ok(img) => img,
        Err(_) => return Ok(false),
    };

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

    Ok(true)
}
