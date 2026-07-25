//! Turning coordinates into place names — the one feature in Hive that has to
//! leave the machine, and therefore the one that is off until you turn it on.
//!
//! There is no offline way to do this without shipping gigabytes of map data, so
//! lookups go to OpenStreetMap's Nominatim service. What is sent is a pair of
//! rounded coordinates and nothing else: no photo, no filename, no identifier.
//! Every answer is cached, so a place is only ever looked up once.

use crate::state::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

/// Nominatim asks for a real identifier and no more than one request a second.
/// Both are conditions of their free service; ignoring them gets users blocked.
const USER_AGENT: &str = "Hive/0.1 (local photo library; https://github.com/mohaneddz/Hive)";
const REQUEST_SPACING_MS: u64 = 1_100;

/// Two decimals — about a kilometre. Enough to name a neighbourhood, coarse
/// enough that the request does not point at a doorstep.
const CACHE_PRECISION: f64 = 100.0;

pub const SETTING_GEOCODING: &str = "geocoding_enabled";

fn cache_key(lat: f64, lon: f64) -> (f64, f64) {
    (
        (lat * CACHE_PRECISION).round() / CACHE_PRECISION,
        (lon * CACHE_PRECISION).round() / CACHE_PRECISION,
    )
}

/// Reads a stored preference. Absent means off — this feature never turns itself on.
pub fn is_enabled(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![SETTING_GEOCODING],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
    .as_deref()
        == Some("1")
}

#[tauri::command]
pub fn get_geocoding_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    Ok(is_enabled(&conn))
}

#[tauri::command]
pub fn set_geocoding_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SETTING_GEOCODING, if enabled { "1" } else { "0" }],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Everything already looked up, so the map can be labelled without touching the
/// network at all.
#[tauri::command]
pub fn get_cached_place_names(
    state: State<'_, AppState>,
) -> Result<Vec<(f64, f64, String)>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT lat, lon, name FROM geocode_cache")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Picks the most useful label Nominatim offers: a village or town if there is
/// one, otherwise the county, otherwise the country.
fn label_from_response(value: &serde_json::Value) -> Option<String> {
    let address = value.get("address")?;
    let pick = |key: &str| address.get(key).and_then(|v| v.as_str()).map(str::to_string);

    let place = pick("city")
        .or_else(|| pick("town"))
        .or_else(|| pick("village"))
        .or_else(|| pick("municipality"))
        .or_else(|| pick("county"));
    let country = pick("country");

    match (place, country) {
        (Some(place), Some(country)) => Some(format!("{place}, {country}")),
        (Some(place), None) => Some(place),
        (None, country) => country,
    }
}

/// Names a batch of coordinates, reading the cache first and only going online
/// for what is genuinely new. Returns `(lat, lon, name)` for everything it could
/// resolve; coordinates it could not name are simply absent.
///
/// Fails fast with a clear message when the feature is off, so a caller can never
/// reach the network by accident.
#[tauri::command]
pub async fn lookup_place_names(
    state: State<'_, AppState>,
    coordinates: Vec<(f64, f64)>,
) -> Result<Vec<(f64, f64, String)>, String> {
    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        if !is_enabled(&conn) {
            return Err("Place-name lookup is turned off in Settings".into());
        }
    }

    let mut resolved: Vec<(f64, f64, String)> = Vec::new();
    let mut missing: Vec<(f64, f64)> = Vec::new();

    {
        let conn = state.conn.lock().map_err(|e| e.to_string())?;
        for (lat, lon) in &coordinates {
            let (key_lat, key_lon) = cache_key(*lat, *lon);
            let hit: Option<String> = conn
                .query_row(
                    "SELECT name FROM geocode_cache WHERE lat = ?1 AND lon = ?2",
                    params![key_lat, key_lon],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;

            match hit {
                Some(name) => resolved.push((key_lat, key_lon, name)),
                None => missing.push((key_lat, key_lon)),
            }
        }
    }

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| e.to_string())?;

    for (lat, lon) in missing {
        let url = format!(
            "https://nominatim.openstreetmap.org/reverse?format=jsonv2&zoom=12&lat={lat}&lon={lon}"
        );

        // Parsed from text rather than `.json()`: reqwest is built without its
        // json feature here, and serde_json is already a dependency.
        let name = match client.get(&url).send().await {
            Ok(response) => response
                .text()
                .await
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|body| label_from_response(&body)),
            // A failed lookup is not an error worth aborting the batch for — the
            // pin simply keeps showing its coordinates.
            Err(_) => None,
        };

        if let Some(name) = name {
            let conn = state.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO geocode_cache (lat, lon, name, looked_up_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(lat, lon) DO UPDATE SET name = excluded.name",
                params![lat, lon, name, chrono::Utc::now().to_rfc3339()],
            )
            .map_err(|e| e.to_string())?;
            resolved.push((lat, lon, name));
        }

        // Their rate limit, honoured rather than tested.
        tokio::time::sleep(std::time::Duration::from_millis(REQUEST_SPACING_MS)).await;
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cache_keys_round_to_about_a_kilometre() {
        let (lat, lon) = cache_key(48.85661, 2.35222);
        assert_eq!(lat, 48.86);
        assert_eq!(lon, 2.35);
    }

    #[test]
    fn nearby_coordinates_share_a_cache_key() {
        assert_eq!(cache_key(48.8566, 2.3522), cache_key(48.8571, 2.3519));
    }

    #[test]
    fn a_city_beats_a_county() {
        let body = json!({ "address": { "city": "Paris", "county": "Île-de-France", "country": "France" } });
        assert_eq!(label_from_response(&body).as_deref(), Some("Paris, France"));
    }

    #[test]
    fn a_village_is_used_when_there_is_no_city() {
        let body = json!({ "address": { "village": "Eguisheim", "country": "France" } });
        assert_eq!(label_from_response(&body).as_deref(), Some("Eguisheim, France"));
    }

    #[test]
    fn the_country_alone_is_better_than_nothing() {
        let body = json!({ "address": { "country": "Iceland" } });
        assert_eq!(label_from_response(&body).as_deref(), Some("Iceland"));
    }

    #[test]
    fn an_empty_answer_names_nothing() {
        assert!(label_from_response(&json!({ "address": {} })).is_none());
        assert!(label_from_response(&json!({})).is_none());
    }
}
