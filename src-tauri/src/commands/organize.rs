//! Automatic grouping that needs no model at all.
//!
//! Timelines, events and trips fall out of two things the indexer already
//! extracted: the capture date and the GPS coordinates. Everything here is
//! arithmetic — sorting by time, measuring gaps, measuring distance. It is
//! listed under "AI" in the task split, but nothing is learned or inferred.

use crate::commands::media::{row_to_media_item, scope_predicate};
use crate::models::{EventGroup, MediaItem, TimelineBucket};
use crate::state::AppState;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use tauri::State;

/// A new event starts when this much time passes with no photo taken.
const DEFAULT_EVENT_GAP_HOURS: i64 = 6;
/// Runs shorter than this are noise, not events.
const DEFAULT_MIN_EVENT_ITEMS: usize = 4;
/// A trip has to last at least this long — an afternoon out is an event, not a trip.
const TRIP_MIN_HOURS: i64 = 20;
/// …and happen at least this far from where most of your photos are taken.
const TRIP_MIN_DISTANCE_KM: f64 = 40.0;

/// Great-circle distance in kilometres. Plain trigonometry — the same formula
/// every mapping library uses.
fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let (dlat, dlon) = ((lat2 - lat1).to_radians(), (lon2 - lon1).to_radians());
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

/// Splits a time-ordered list wherever the gap to the next entry exceeds
/// `gap_hours`, then drops runs that are too small to be interesting.
fn split_on_gaps<T: Clone>(
    entries: &[(DateTime<Utc>, T)],
    gap_hours: i64,
    min_items: usize,
) -> Vec<Vec<(DateTime<Utc>, T)>> {
    let mut runs: Vec<Vec<(DateTime<Utc>, T)>> = Vec::new();
    let mut current: Vec<(DateTime<Utc>, T)> = Vec::new();

    for entry in entries {
        match current.last() {
            Some((previous, _))
                if (entry.0 - *previous).num_minutes() > gap_hours * 60 =>
            {
                runs.push(std::mem::take(&mut current));
            }
            _ => {}
        }
        current.push(entry.clone());
    }
    if !current.is_empty() {
        runs.push(current);
    }

    runs.retain(|run| run.len() >= min_items);
    runs
}

/// The capture date Hive sorts by: a hand-corrected date wins over EXIF, which
/// wins over the file's own creation date.
const EFFECTIVE_DATE: &str = "COALESCE(taken_at_override, taken_at, created_at)";

fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            // EXIF dates arrive as "YYYY-MM-DD HH:MM:SS" with no zone.
            chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|naive| naive.and_utc())
        })
}

/// The bucket key `list_media_in_bucket` recognizes for photos with no usable date at all.
pub const UNSPECIFIED_BUCKET_KEY: &str = "unspecified";

/// Photo counts per year, month or day, newest first — the raw material for a
/// timeline view. Undated photos are never grouped into `strftime` buckets, so
/// they come back as one trailing "unspecified" bucket instead of vanishing.
#[tauri::command]
pub fn get_timeline(
    state: State<'_, AppState>,
    granularity: Option<String>,
) -> Result<Vec<TimelineBucket>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    // Fixed set, never built from caller text.
    let (format, label_len) = match granularity.as_deref() {
        Some("day") => ("%Y-%m-%d", 10),
        Some("month") => ("%Y-%m", 7),
        _ => ("%Y", 4),
    };

    let live = scope_predicate(None);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT strftime('{format}', {EFFECTIVE_DATE}) AS bucket,
                    COUNT(*),
                    MIN({EFFECTIVE_DATE}),
                    MAX({EFFECTIVE_DATE})
             FROM media_items
             WHERE {live} AND {EFFECTIVE_DATE} IS NOT NULL
             GROUP BY bucket
             HAVING bucket IS NOT NULL
             ORDER BY bucket DESC"
        ))
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    let mut buckets = Vec::with_capacity(rows.len() + 1);
    for (key, count, start, end) in rows {
        // One cover query per bucket; there are only ever a handful on screen.
        let cover_media_id: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT id FROM media_items
                     WHERE {live} AND strftime('{format}', {EFFECTIVE_DATE}) = ?1
                     ORDER BY {EFFECTIVE_DATE} DESC LIMIT 1"
                ),
                params![key],
                |r| r.get(0),
            )
            .ok();

        buckets.push(TimelineBucket {
            label: key.chars().take(label_len).collect(),
            key,
            count,
            cover_media_id,
            start,
            end,
        });
    }

    let undated_count: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM media_items WHERE {live} AND {EFFECTIVE_DATE} IS NULL"),
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if undated_count > 0 {
        let cover_media_id: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT id FROM media_items WHERE {live} AND {EFFECTIVE_DATE} IS NULL
                     ORDER BY indexed_at DESC LIMIT 1"
                ),
                [],
                |r| r.get(0),
            )
            .ok();

        buckets.push(TimelineBucket {
            key: UNSPECIFIED_BUCKET_KEY.to_string(),
            label: "Unspecified".to_string(),
            count: undated_count,
            cover_media_id,
            start: String::new(),
            end: String::new(),
        });
    }

    Ok(buckets)
}

/// Every photo in one timeline bucket, newest first. The synthetic
/// `UNSPECIFIED_BUCKET_KEY` bucket has no date to sort by, so it reads back by
/// index time instead.
#[tauri::command]
pub fn list_media_in_bucket(
    state: State<'_, AppState>,
    granularity: Option<String>,
    key: String,
    limit: i64,
) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let live = scope_predicate(None);

    let ids: Vec<String> = if key == UNSPECIFIED_BUCKET_KEY {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id FROM media_items
                 WHERE {live} AND {EFFECTIVE_DATE} IS NULL
                 ORDER BY indexed_at DESC LIMIT ?1"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|id| id.ok())
            .collect();
        rows
    } else {
        let format = match granularity.as_deref() {
            Some("day") => "%Y-%m-%d",
            Some("month") => "%Y-%m",
            _ => "%Y",
        };
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id FROM media_items
                 WHERE {live} AND strftime('{format}', {EFFECTIVE_DATE}) = ?1
                 ORDER BY {EFFECTIVE_DATE} DESC LIMIT ?2"
            ))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![key, limit], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|id| id.ok())
            .collect();
        rows
    };

    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Everything the grouping needs about one photo, read once.
#[derive(Clone)]
struct DatedItem {
    id: String,
    coords: Option<(f64, f64)>,
}

fn load_dated_items(conn: &Connection) -> Result<Vec<(DateTime<Utc>, DatedItem)>, String> {
    let live = scope_predicate(None);
    let mut stmt = conn
        .prepare(&format!(
            "SELECT m.id, {EFFECTIVE_DATE}, e.gps_lat, e.gps_lon
             FROM media_items m
             LEFT JOIN exif_data e ON e.media_id = m.id
             WHERE {live} AND {EFFECTIVE_DATE} IS NOT NULL
             ORDER BY {EFFECTIVE_DATE} ASC"
        ))
        .map_err(|e| e.to_string())?;

    let rows: Vec<(String, String, Option<f64>, Option<f64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    Ok(rows
        .into_iter()
        .filter_map(|(id, raw_date, lat, lon)| {
            let taken = parse_date(&raw_date)?;
            let coords = match (lat, lon) {
                (Some(lat), Some(lon)) => Some((lat, lon)),
                _ => None,
            };
            Some((taken, DatedItem { id, coords }))
        })
        .collect())
}

fn build_group(run: &[(DateTime<Utc>, DatedItem)], is_trip: bool, distance_km: f64) -> EventGroup {
    let located: Vec<(f64, f64)> = run.iter().filter_map(|(_, item)| item.coords).collect();
    let centroid = if located.is_empty() {
        None
    } else {
        let count = located.len() as f64;
        Some((
            located.iter().map(|(lat, _)| lat).sum::<f64>() / count,
            located.iter().map(|(_, lon)| lon).sum::<f64>() / count,
        ))
    };

    let start = run.first().map(|(at, _)| *at).unwrap_or_else(Utc::now);
    let end = run.last().map(|(at, _)| *at).unwrap_or(start);

    EventGroup {
        id: format!("{}_{}", start.timestamp(), end.timestamp()),
        start: start.to_rfc3339(),
        end: end.to_rfc3339(),
        count: run.len() as i64,
        // The last frame of a run is usually the most representative one.
        cover_media_id: run.last().map(|(_, item)| item.id.clone()).unwrap_or_default(),
        media_ids: run.iter().map(|(_, item)| item.id.clone()).collect(),
        lat: centroid.map(|(lat, _)| lat),
        lon: centroid.map(|(_, lon)| lon),
        is_trip,
        distance_km,
    }
}

/// Bursts of activity: photos taken close together in time, with a quiet gap on
/// either side. A birthday, a hike, an afternoon at the beach.
#[tauri::command]
pub fn detect_events(
    state: State<'_, AppState>,
    gap_hours: Option<i64>,
    min_items: Option<usize>,
) -> Result<Vec<EventGroup>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let items = load_dated_items(&conn)?;

    let runs = split_on_gaps(
        &items,
        gap_hours.unwrap_or(DEFAULT_EVENT_GAP_HOURS).clamp(1, 240),
        min_items.unwrap_or(DEFAULT_MIN_EVENT_ITEMS).max(2),
    );

    let mut events: Vec<EventGroup> = runs
        .iter()
        .map(|run| build_group(run, false, 0.0))
        .collect();
    events.reverse(); // newest first
    Ok(events)
}

/// Trips are events that went somewhere. Two conditions, both measured:
/// the run lasts at least a night, and its centre of gravity sits far from where
/// most of your photos are taken.
#[tauri::command]
pub fn detect_trips(
    state: State<'_, AppState>,
    min_distance_km: Option<f64>,
) -> Result<Vec<EventGroup>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let items = load_dated_items(&conn)?;
    let threshold = min_distance_km.unwrap_or(TRIP_MIN_DISTANCE_KM).max(1.0);

    // "Home" is the busiest half-degree cell — roughly where you live, inferred
    // from nothing more than how many photos you took there.
    let home: Option<(f64, f64)> = {
        let mut cells: std::collections::HashMap<(i64, i64), (f64, f64, i64)> =
            std::collections::HashMap::new();
        for (_, item) in &items {
            if let Some((lat, lon)) = item.coords {
                let cell = ((lat * 2.0).round() as i64, (lon * 2.0).round() as i64);
                let entry = cells.entry(cell).or_insert((0.0, 0.0, 0));
                entry.0 += lat;
                entry.1 += lon;
                entry.2 += 1;
            }
        }
        cells
            .into_values()
            .max_by_key(|(_, _, count)| *count)
            .map(|(lat_sum, lon_sum, count)| (lat_sum / count as f64, lon_sum / count as f64))
    };

    let Some((home_lat, home_lon)) = home else {
        // Without a single geotagged photo there is no "away" to speak of.
        return Ok(vec![]);
    };

    // A day-long gap separates trips; a six-hour gap would cut one holiday into
    // a dozen pieces.
    let runs = split_on_gaps(&items, 24, DEFAULT_MIN_EVENT_ITEMS);

    let mut trips: Vec<EventGroup> = runs
        .iter()
        .filter_map(|run| {
            let start = run.first()?.0;
            let end = run.last()?.0;
            if (end - start).num_hours() < TRIP_MIN_HOURS {
                return None;
            }

            let located: Vec<(f64, f64)> = run.iter().filter_map(|(_, item)| item.coords).collect();
            if located.is_empty() {
                return None;
            }
            let count = located.len() as f64;
            let lat = located.iter().map(|(lat, _)| lat).sum::<f64>() / count;
            let lon = located.iter().map(|(_, lon)| lon).sum::<f64>() / count;

            let distance = haversine_km(home_lat, home_lon, lat, lon);
            (distance >= threshold).then(|| build_group(run, true, distance))
        })
        .collect();

    trips.reverse();
    Ok(trips)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(hour * 3600, 0).unwrap()
    }

    #[test]
    fn haversine_matches_a_known_distance() {
        // Paris → London, about 344 km.
        let km = haversine_km(48.8566, 2.3522, 51.5074, -0.1278);
        assert!((km - 344.0).abs() < 5.0, "got {km} km");
    }

    #[test]
    fn haversine_is_zero_for_the_same_point() {
        assert!(haversine_km(45.0, 3.0, 45.0, 3.0) < 0.001);
    }

    #[test]
    fn gaps_split_runs_and_small_runs_are_dropped() {
        let entries = vec![
            (at(0), 'a'),
            (at(1), 'b'),
            (at(2), 'c'),
            // 10-hour gap starts a new run
            (at(12), 'd'),
            (at(13), 'e'),
        ];
        let runs = split_on_gaps(&entries, 6, 3);
        assert_eq!(runs.len(), 1, "the two-photo run is below the minimum");
        assert_eq!(runs[0].len(), 3);
    }

    #[test]
    fn a_gap_exactly_on_the_threshold_does_not_split() {
        let entries = vec![(at(0), 'a'), (at(6), 'b'), (at(7), 'c')];
        let runs = split_on_gaps(&entries, 6, 2);
        assert_eq!(runs.len(), 1, "6h is not *more* than a 6h gap");
    }

    #[test]
    fn everything_in_one_burst_stays_one_run() {
        let entries: Vec<_> = (0..8).map(|i| (at(i), i)).collect();
        let runs = split_on_gaps(&entries, 6, 4);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].len(), 8);
    }

    #[test]
    fn exif_style_dates_without_a_timezone_parse() {
        assert!(parse_date("2024-06-15 10:30:00").is_some());
        assert!(parse_date("2024-06-15T10:30:00Z").is_some());
        assert!(parse_date("not a date").is_none());
    }
}
