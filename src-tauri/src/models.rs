use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub path: String,
    pub name: String,
    pub is_watched: bool,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub exif: Option<ExifData>,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPage {
    pub items: Vec<MediaItem>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub folder_id: String,
    pub scanned: i64,
    pub added: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub current: i64,
    pub total: i64,
    pub message: Option<String>,
}
