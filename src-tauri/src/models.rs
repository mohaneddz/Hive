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
    /// 0..1 from the sensitive-content model, or None if never scanned.
    /// Travels with every item so the grid can cover a photo before showing it.
    pub nsfw_score: Option<f64>,
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
    /// 0 leaves colour alone, 1 is fully black and white.
    #[serde(default)]
    pub grayscale: f32,
    /// 0 leaves colour alone, 1 is a full sepia tone.
    #[serde(default)]
    pub sepia: f32,
    /// −1 cools the image towards blue, +1 warms it towards orange.
    #[serde(default)]
    pub temperature: f32,
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

/// One slice of the timeline: a year, a month or a day.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBucket {
    /// The value `strftime` produced, e.g. "2024" or "2024-06".
    pub key: String,
    pub label: String,
    pub count: i64,
    pub cover_media_id: Option<String>,
    pub start: String,
    pub end: String,
}

/// A run of photos taken close together. `isTrip` marks the ones that also
/// happened far from home; `distanceKm` is how far.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventGroup {
    pub id: String,
    pub start: String,
    pub end: String,
    pub count: i64,
    pub cover_media_id: String,
    pub media_ids: Vec<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub is_trip: bool,
    pub distance_km: f64,
}

/// A photo and its sharpness score. Lower means softer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlurryItem {
    pub item: MediaItem,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlurReport {
    /// How many photos were measured during this run.
    pub scanned: i64,
    /// How many have a score at all, including from earlier runs.
    pub measured: i64,
    pub threshold: f64,
    pub items: Vec<BlurryItem>,
}

/// State of the thumbnail cache after a limit was applied. `limitBytes` of 0
/// means no ceiling is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheReport {
    pub used_bytes: i64,
    pub limit_bytes: i64,
    pub freed_bytes: i64,
    pub removed: i64,
}

/// What the app does with the sensitive-content score.
///
/// The threshold lives here rather than in two constants so the blur in the grid
/// and the decision to file a photo away can never disagree about what "sensitive"
/// means.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NsfwPolicy {
    /// Score at or above which a photo is treated as sensitive, 0..1.
    pub threshold: f64,
    /// Whether scoring also moves those photos out of the library into Hidden.
    pub auto_hide: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub exported: i64,
    pub skipped: i64,
    pub destination: String,
}
