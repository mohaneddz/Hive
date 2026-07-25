use crate::commands::media::row_to_media_item;
use crate::models::{MediaItem, PlaceGroup};
use crate::state::AppState;
use rusqlite::params;
use tauri::State;

/// How many decimals of latitude/longitude two photos must share to land in the
/// same pin. 1 decimal is roughly a city, 2 a neighbourhood, 3 a street.
const DEFAULT_PRECISION: i64 = 1;

/// Same rule as `media::scope_predicate("library")`, written with the `m.` alias
/// these joined queries need.
const LIVE_MEDIA: &str = "m.is_trashed = 0 AND m.is_hidden = 0 AND m.is_archived = 0";

/// Clusters every geotagged photo onto a grid and returns one pin per cell.
/// Coordinates come back averaged, so a pin sits in the middle of its group
/// rather than on the corner of the grid cell.
#[tauri::command]
pub fn list_places(
    state: State<'_, AppState>,
    precision: Option<i64>,
) -> Result<Vec<PlaceGroup>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let precision = precision.unwrap_or(DEFAULT_PRECISION).clamp(0, 4);
    let live = LIVE_MEDIA;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT ROUND(e.gps_lat, ?1) AS cell_lat,
                    ROUND(e.gps_lon, ?1) AS cell_lon,
                    COUNT(*),
                    AVG(e.gps_lat),
                    AVG(e.gps_lon),
                    MIN(COALESCE(m.taken_at, m.created_at)),
                    MAX(COALESCE(m.taken_at, m.created_at))
             FROM exif_data e
             JOIN media_items m ON m.id = e.media_id
             WHERE {live} AND e.gps_lat IS NOT NULL AND e.gps_lon IS NOT NULL
             GROUP BY cell_lat, cell_lon
             ORDER BY COUNT(*) DESC"
        ))
        .map_err(|e| e.to_string())?;

    let raw: Vec<(f64, f64, i64, f64, f64, Option<String>, Option<String>)> = stmt
        .query_map(params![precision], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Fetching the cover per group keeps the grouping query readable; there are
    // only ever as many extra queries as there are pins on screen.
    let mut places = Vec::with_capacity(raw.len());
    for (cell_lat, cell_lon, count, lat, lon, earliest, latest) in raw {
        let cover_media_id: String = conn
            .query_row(
                &format!(
                    "SELECT m.id FROM media_items m
                     JOIN exif_data e ON e.media_id = m.id
                     WHERE {live} AND ROUND(e.gps_lat, ?1) = ?2 AND ROUND(e.gps_lon, ?1) = ?3
                     ORDER BY COALESCE(m.taken_at, m.created_at) DESC LIMIT 1"
                ),
                params![precision, cell_lat, cell_lon],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;

        places.push(PlaceGroup {
            id: format!("{cell_lat}_{cell_lon}"),
            lat,
            lon,
            count,
            cover_media_id,
            earliest,
            latest,
        });
    }

    Ok(places)
}

/// Every photo belonging to one pin, newest first.
#[tauri::command]
pub fn list_media_at_place(
    state: State<'_, AppState>,
    lat: f64,
    lon: f64,
    precision: Option<i64>,
    limit: i64,
) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let precision = precision.unwrap_or(DEFAULT_PRECISION).clamp(0, 4);
    let live = LIVE_MEDIA;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT m.id FROM media_items m
             JOIN exif_data e ON e.media_id = m.id
             WHERE {live}
               AND ROUND(e.gps_lat, ?1) = ROUND(?2, ?1)
               AND ROUND(e.gps_lon, ?1) = ROUND(?3, ?1)
             ORDER BY COALESCE(m.taken_at, m.created_at) DESC LIMIT ?4"
        ))
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![precision, lat, lon, limit], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|id| id.ok())
        .collect();

    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
