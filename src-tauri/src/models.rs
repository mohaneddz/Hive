use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub path: String,
    pub name: String,
    pub is_watched: bool,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifData {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<i64>,
    pub f_number: Option<f64>,
    pub exposure_time: Option<String>,
    pub focal_length: Option<f64>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub folder_id: String,
    pub path: String,
    pub filename: String,
    pub hash: String,
    pub size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub mime_type: String,
    pub media_type: String,
    pub taken_at: Option<String>,
    pub created_at: String,
    pub modified_at: String,
    pub indexed_at: String,
    pub is_favorite: bool,
    pub is_trashed: bool,
    pub trashed_at: Option<String>,
    pub is_hidden: bool,
    pub is_archived: bool,
    pub last_viewed_at: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    /// A capture date the user corrected by hand; wins over `taken_at`.
    pub taken_at_override: Option<String>,
    pub edited_at: Option<String>,
    pub exif: Option<ExifData>,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPage {
    pub items: Vec<MediaItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub current: i64,
    pub total: i64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStats {
    pub folder: Folder,
    pub item_count: i64,
    pub cover_media_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceCluster {
    pub lat: f64,
    pub lon: f64,
    pub count: i64,
    pub cover_media_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub group_id: String,
    pub items: Vec<MediaItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonSummary {
    pub id: String,
    pub name: Option<String>,
    pub face_count: i64,
    pub cover_face_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub total_items: i64,
    pub total_bytes: i64,
    pub favorites: i64,
    pub trashed: i64,
    pub image_count: i64,
    pub video_count: i64,
    pub album_count: i64,
    pub place_count: i64,
    pub hidden_count: i64,
    pub archived_count: i64,
    pub folder_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub cover_media_id: Option<String>,
    pub item_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// One cluster of photos shot close to each other. Coordinates are the average
/// of the cluster, so the marker sits in the middle of the group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceGroup {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub count: i64,
    pub cover_media_id: String,
    pub earliest: Option<String>,
    pub latest: Option<String>,
}

/// A row in the file explorer: either a directory to descend into, or a media file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub media_count: i64,
    pub indexed_count: i64,
    pub is_watched: bool,
}

/// One set of adjustments, as the editor sends them.
///
/// Applied in a fixed order — **rotate → flip → crop → colour** — and the browser
/// preview follows exactly the same order, so what you see while dragging is what
/// gets written.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOps {
    /// 0, 90, 180 or 270, clockwise.
    pub rotation: i32,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    /// Fractions of the rotated image, 0..1. `None` keeps the whole frame.
    pub crop: Option<CropRect>,
    /// 1.0 leaves the channel untouched. Same maths as the CSS filters of the
    /// same name, which is what makes the live preview faithful.
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Outcome of a batch operation. `bytesBefore`/`bytesAfter` are only meaningful
/// for compression and conversion, where the point is the size difference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReport {
    pub processed: i64,
    pub skipped: i64,
    pub failed: i64,
    pub bytes_before: i64,
    pub bytes_after: i64,
    pub destination: Option<String>,
    pub first_error: Option<String>,
}

/// One line of the rename preview, so nothing is renamed sight unseen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePreview {
    pub media_id: String,
    pub from: String,
    pub to: String,
    /// True when the target name already exists on disk or repeats inside the batch.
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub bytes: i64,
    pub created_at: String,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderUsage {
    pub folder_id: String,
    pub name: String,
    pub path: String,
    pub item_count: i64,
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub total_items: i64,
    pub image_count: i64,
    pub video_count: i64,
    pub original_bytes: i64,
    pub thumbnail_bytes: i64,
    pub database_bytes: i64,
    pub by_folder: Vec<FolderUsage>,
}

/// Result of the library health sweep: rows whose file vanished, and rows whose
/// file is still there but can no longer be decoded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryHealth {
    pub checked: i64,
    pub missing: Vec<MediaItem>,
    pub broken: Vec<MediaItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub exported: i64,
    pub skipped: i64,
    pub destination: String,
}
