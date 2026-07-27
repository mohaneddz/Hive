//! Smart albums: rule-based albums that automatically include matching photos.
//!
//! A smart album is defined by one or more rules (tag, person, date range, media
//! type, score...). When the album is opened, the matching photos are computed on
//! the fly — nothing is stored in a join table like regular albums.
//!
//! Rules are joined according to the album's `match_type`: `all` demands every
//! rule, `any` a single one. Any operator can be prefixed with `not_` to invert
//! it, so "the whole trip except the blurry shots" is expressible.
//!
//! This module also generates suggested smart albums from the data already in the
//! library (top tags, named people, best-rated photos).

use crate::commands::media::row_to_media_item;
use crate::models::MediaItem;
use crate::state::AppState;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Every rule kind [`rule_to_sql`] understands, with the operators it accepts.
/// It lives next to the parser so the two cannot drift, and it is read back out
/// to build the error message when a rule is rejected.
pub const SUPPORTED_RULES: &[(&str, &[&str])] = &[
    ("tag", &["is", "contains"]),
    ("person", &["is"]),
    ("media_type", &["is"]),
    ("favorite", &["is"]),
    ("date_range", &["between"]),
    ("place", &["is", "contains"]),
    ("camera", &["is", "contains"]),
    ("caption", &["contains"]),
    ("filename", &["contains"]),
    ("aesthetic", &["at_least", "at_most"]),
    ("blur", &["at_least", "at_most"]),
    ("nsfw", &["at_least", "at_most"]),
];

/// The two ways a set of rules can be combined.
pub const MATCH_ALL: &str = "all";
pub const MATCH_ANY: &str = "any";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartAlbumRule {
    /// One of the kinds listed in [`SUPPORTED_RULES`].
    pub kind: String,
    /// One of the operators that kind accepts, optionally prefixed with `not_`.
    pub operator: String,
    /// The value to match: tag name, person_id, "start|end", a number, etc.
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartAlbum {
    pub id: String,
    pub name: String,
    pub rules: Vec<SmartAlbumRule>,
    /// [`MATCH_ALL`] or [`MATCH_ANY`].
    pub match_type: String,
    pub cover_media_id: Option<String>,
    pub item_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartAlbumSuggestion {
    pub name: String,
    pub rules: Vec<SmartAlbumRule>,
    pub match_type: String,
    pub preview_count: i64,
}

/// `%` and `_` are wildcards for LIKE. A filename search for "100_0123" has to
/// look for that literal text, so they are escaped here and every LIKE clause
/// declares `ESCAPE '\'`.
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn contains_pattern(value: &str) -> Value {
    Value::Text(format!("%{}%", escape_like(value)))
}

fn number(rule: &SmartAlbumRule) -> Result<Value, String> {
    rule.value
        .trim()
        .parse::<f64>()
        .map(Value::Real)
        .map_err(|_| {
            format!(
                "rule \"{}\" expects a number, got \"{}\"",
                rule.kind, rule.value
            )
        })
}

/// Names the problem with a rule this build cannot translate, listing what would
/// have worked instead.
fn unsupported(rule: &SmartAlbumRule) -> String {
    match SUPPORTED_RULES.iter().find(|(kind, _)| *kind == rule.kind) {
        Some((kind, operators)) => format!(
            "rule \"{kind}\" has no operator \"{}\" — accepted: {}",
            rule.operator,
            operators.join(", ")
        ),
        None => format!(
            "unknown rule \"{}\" — accepted: {}",
            rule.kind,
            SUPPORTED_RULES
                .iter()
                .map(|(kind, _)| *kind)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Builds the WHERE fragment for a single rule, plus the values to bind to it.
///
/// Returns `Err` for anything it cannot translate. That is the point: the first
/// version returned `None` and the caller skipped the rule, so one typo in a
/// saved album silently widened it — "beach photos of 2024" quietly became "all
/// photos of 2024", with nothing on screen to say so.
fn rule_to_sql(rule: &SmartAlbumRule) -> Result<(String, Vec<Value>), String> {
    // Negation is peeled off once here rather than doubling every arm below.
    let (operator, negated) = match rule.operator.strip_prefix("not_") {
        Some(rest) => (rest, true),
        None => (rule.operator.as_str(), false),
    };

    let (clause, binds): (String, Vec<Value>) = match (rule.kind.as_str(), operator) {
        ("tag", "is") => (
            "m.id IN (SELECT media_id FROM tags WHERE tag = ?)".into(),
            vec![Value::Text(rule.value.clone())],
        ),
        ("tag", "contains") => (
            "m.id IN (SELECT media_id FROM tags WHERE tag LIKE ? ESCAPE '\\')".into(),
            vec![contains_pattern(&rule.value)],
        ),
        ("person", "is") => (
            "m.id IN (SELECT media_id FROM faces WHERE person_id = ?)".into(),
            vec![Value::Text(rule.value.clone())],
        ),
        ("media_type", "is") => (
            "m.media_type = ?".into(),
            vec![Value::Text(rule.value.clone())],
        ),
        ("favorite", "is") => ("m.is_favorite = 1".into(), vec![]),
        ("date_range", "between") => {
            let (start, end) = rule.value.split_once('|').ok_or_else(|| {
                format!(
                    "rule \"date_range\" expects \"start|end\", got \"{}\"",
                    rule.value
                )
            })?;
            (
                "COALESCE(m.taken_at_override, m.taken_at, m.created_at) BETWEEN ? AND ?".into(),
                vec![Value::Text(start.into()), Value::Text(end.into())],
            )
        }
        // Place names are cached at two decimals (~1 km) by the geocoder, so the
        // photo's own coordinates are rounded the same way to line the two up.
        ("place", "is") | ("place", "contains") => {
            let pattern = if operator == "is" {
                Value::Text(escape_like(&rule.value))
            } else {
                contains_pattern(&rule.value)
            };
            (
                "m.id IN (SELECT e.media_id FROM exif_data e
                      JOIN geocode_cache g
                        ON ROUND(e.gps_lat, 2) = g.lat AND ROUND(e.gps_lon, 2) = g.lon
                      WHERE g.name LIKE ? ESCAPE '\\')"
                    .into(),
                vec![pattern],
            )
        }
        ("camera", "is") => (
            "m.id IN (SELECT media_id FROM exif_data WHERE camera_model = ?)".into(),
            vec![Value::Text(rule.value.clone())],
        ),
        ("camera", "contains") => (
            "m.id IN (SELECT media_id FROM exif_data WHERE camera_model LIKE ? ESCAPE '\\')".into(),
            vec![contains_pattern(&rule.value)],
        ),
        ("caption", "contains") => (
            "m.id IN (SELECT media_id FROM captions WHERE text LIKE ? ESCAPE '\\')".into(),
            vec![contains_pattern(&rule.value)],
        ),
        ("filename", "contains") => (
            "m.filename LIKE ? ESCAPE '\\'".into(),
            vec![contains_pattern(&rule.value)],
        ),
        ("aesthetic", "at_least") => ("m.aesthetic_score >= ?".into(), vec![number(rule)?]),
        ("aesthetic", "at_most") => ("m.aesthetic_score <= ?".into(), vec![number(rule)?]),
        ("blur", "at_least") => ("m.blur_score >= ?".into(), vec![number(rule)?]),
        ("blur", "at_most") => ("m.blur_score <= ?".into(), vec![number(rule)?]),
        ("nsfw", "at_least") => ("m.nsfw_score >= ?".into(), vec![number(rule)?]),
        ("nsfw", "at_most") => ("m.nsfw_score <= ?".into(), vec![number(rule)?]),
        _ => return Err(unsupported(rule)),
    };

    // A score that was never computed is NULL, and `NOT (NULL >= 7)` is itself
    // NULL, which SQLite drops. "not rated at least 7" plainly covers photos with
    // no rating at all, so an undecidable comparison negates to true.
    Ok(if negated {
        (format!("IFNULL(NOT ({clause}), 1)"), binds)
    } else {
        (clause, binds)
    })
}

/// The SQL keyword that joins the rules of an album.
fn joiner(match_type: &str) -> Result<&'static str, String> {
    match match_type {
        MATCH_ALL => Ok(" AND "),
        MATCH_ANY => Ok(" OR "),
        other => Err(format!(
            "match type must be \"{MATCH_ALL}\" or \"{MATCH_ANY}\", got \"{other}\""
        )),
    }
}

/// Executes the rules and returns matching media IDs, newest first.
fn query_smart_album(
    conn: &Connection,
    rules: &[SmartAlbumRule],
    match_type: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let separator = joiner(match_type)?;
    if rules.is_empty() {
        return Ok(vec![]);
    }

    let mut clauses = Vec::with_capacity(rules.len());
    let mut binds: Vec<Value> = Vec::new();
    for rule in rules {
        let (clause, values) = rule_to_sql(rule)?;
        clauses.push(clause);
        binds.extend(values);
    }

    // The parentheses are load-bearing: without them `is_trashed = 0 AND a OR b`
    // would let an `any` album pull photos back out of the trash.
    let sql = format!(
        "SELECT m.id FROM media_items m
         WHERE m.is_trashed = 0 AND ({})
         ORDER BY COALESCE(m.taken_at_override, m.taken_at, m.created_at) DESC
         LIMIT ?",
        clauses.join(separator)
    );
    binds.push(Value::Integer(limit));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let ids = stmt
        .query_map(params_from_iter(binds.iter()), |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(ids)
}

#[tauri::command]
pub fn create_smart_album(
    state: State<'_, AppState>,
    name: String,
    rules: Vec<SmartAlbumRule>,
    match_type: Option<String>,
) -> Result<SmartAlbum, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let match_type = match_type.unwrap_or_else(|| MATCH_ALL.to_string());
    if rules.is_empty() {
        return Err("a smart album needs at least one rule".into());
    }

    // Run the rules before the INSERT, so an album that could never load is never
    // saved, and the cover is the newest match rather than an arbitrary photo.
    let ids = query_smart_album(&conn, &rules, &match_type, 1000)?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let rules_json = serde_json::to_string(&rules).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO smart_albums (id, name, rules, match_type, cover_media_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, name, rules_json, match_type, ids.first(), now],
    )
    .map_err(|e| e.to_string())?;

    Ok(SmartAlbum {
        id,
        name,
        rules,
        match_type,
        cover_media_id: ids.first().cloned(),
        item_count: ids.len() as i64,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub fn list_smart_albums(state: State<'_, AppState>) -> Result<Vec<SmartAlbum>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, rules, match_type, cover_media_id, created_at, updated_at
             FROM smart_albums ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    type Row = (String, String, String, String, Option<String>, String, String);
    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();

    Ok(rows
        .into_iter()
        .map(
            |(id, name, rules_json, match_type, cover_media_id, created_at, updated_at)| {
                let rules: Vec<SmartAlbumRule> =
                    serde_json::from_str(&rules_json).unwrap_or_default();
                // Counts are recomputed live, which is the whole point of a smart
                // album. A rule this build cannot translate counts as zero rather
                // than failing the entire list; opening the album reports why.
                let item_count = query_smart_album(&conn, &rules, &match_type, 10000)
                    .map(|ids| ids.len() as i64)
                    .unwrap_or(0);
                SmartAlbum {
                    id,
                    name,
                    rules,
                    match_type,
                    cover_media_id,
                    item_count,
                    created_at,
                    updated_at,
                }
            },
        )
        .collect())
}

#[tauri::command]
pub fn get_smart_album_media(
    state: State<'_, AppState>,
    album_id: String,
    limit: i64,
) -> Result<Vec<MediaItem>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let (rules_json, match_type): (String, String) = conn
        .query_row(
            "SELECT rules, match_type FROM smart_albums WHERE id = ?1",
            params![album_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let rules: Vec<SmartAlbumRule> =
        serde_json::from_str(&rules_json).map_err(|e| e.to_string())?;

    let ids = query_smart_album(&conn, &rules, &match_type, limit)?;
    ids.iter()
        .map(|id| row_to_media_item(&conn, id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_smart_album(state: State<'_, AppState>, album_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM smart_albums WHERE id = ?1", params![album_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// A suggestion is always one rule, so `match_type` never matters — it is filled
/// in anyway so the album created from it is well-formed.
fn single_rule_suggestion(
    name: String,
    kind: &str,
    operator: &str,
    value: String,
    count: i64,
) -> SmartAlbumSuggestion {
    SmartAlbumSuggestion {
        name,
        rules: vec![SmartAlbumRule {
            kind: kind.to_string(),
            operator: operator.to_string(),
            value,
        }],
        match_type: MATCH_ALL.to_string(),
        preview_count: count,
    }
}

/// Generates smart album suggestions based on what's already in the library.
#[tauri::command]
pub fn suggest_smart_albums(state: State<'_, AppState>) -> Result<Vec<SmartAlbumSuggestion>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut suggestions: Vec<SmartAlbumSuggestion> = Vec::new();

    // Suggest albums for top tags (minimum 5 photos).
    let mut tag_stmt = conn
        .prepare(
            "SELECT t.tag, COUNT(*) as cnt FROM tags t
             JOIN media_items m ON m.id = t.media_id
             WHERE m.is_trashed = 0
             GROUP BY t.tag HAVING cnt >= 5
             ORDER BY cnt DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;
    let tag_suggestions: Vec<(String, i64)> = tag_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    for (tag, count) in tag_suggestions {
        suggestions.push(single_rule_suggestion(
            capitalize(&tag),
            "tag",
            "is",
            tag,
            count,
        ));
    }

    // Suggest albums for named people (minimum 3 photos).
    let mut people_stmt = conn
        .prepare(
            "SELECT p.id, p.name, COUNT(DISTINCT f.media_id) as cnt
             FROM people p JOIN faces f ON f.person_id = p.id
             JOIN media_items m ON m.id = f.media_id
             WHERE p.name IS NOT NULL AND m.is_trashed = 0
             GROUP BY p.id HAVING cnt >= 3
             ORDER BY cnt DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;
    let people_suggestions: Vec<(String, String, i64)> = people_stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    for (person_id, name, count) in people_suggestions {
        suggestions.push(single_rule_suggestion(
            format!("Photos with {name}"),
            "person",
            "is",
            person_id,
            count,
        ));
    }

    // The aesthetic ranking is otherwise only reachable from its own page; as a
    // smart album it keeps updating as new photos are scored.
    let best_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM media_items
             WHERE aesthetic_score >= 7.0 AND is_trashed = 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if best_count >= 5 {
        suggestions.push(single_rule_suggestion(
            "Best photos".to_string(),
            "aesthetic",
            "at_least",
            "7".to_string(),
            best_count,
        ));
    }

    // Suggest a "Favorites" smart album if there are favorites.
    let fav_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM media_items WHERE is_favorite = 1 AND is_trashed = 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if fav_count >= 3 {
        suggestions.push(single_rule_suggestion(
            "Favorites".to_string(),
            "favorite",
            "is",
            "true".to_string(),
            fav_count,
        ));
    }

    // Suggest a "Videos" smart album if there are videos.
    let video_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM media_items WHERE media_type = 'video' AND is_trashed = 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if video_count >= 3 {
        suggestions.push(single_rule_suggestion(
            "Videos".to_string(),
            "media_type",
            "is",
            "video".to_string(),
            video_count,
        ));
    }

    // The camera someone shoots most with is usually the one worth a shelf.
    let top_camera: Option<(String, i64)> = conn
        .query_row(
            "SELECT e.camera_model, COUNT(*) as cnt FROM exif_data e
             JOIN media_items m ON m.id = e.media_id
             WHERE e.camera_model IS NOT NULL AND e.camera_model <> '' AND m.is_trashed = 0
             GROUP BY e.camera_model ORDER BY cnt DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((model, count)) = top_camera {
        if count >= 10 {
            suggestions.push(single_rule_suggestion(
                format!("Shot on {model}"),
                "camera",
                "is",
                model,
                count,
            ));
        }
    }

    Ok(suggestions)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: &str, operator: &str, value: &str) -> SmartAlbumRule {
        SmartAlbumRule {
            kind: kind.to_string(),
            operator: operator.to_string(),
            value: value.to_string(),
        }
    }

    /// Just enough schema for the clauses under test, in the style of the other
    /// command modules. Three photos: a sharp beach one, a blurry beach one, and
    /// a mountain one that was never scored.
    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE media_items (
                id TEXT PRIMARY KEY, filename TEXT NOT NULL, media_type TEXT NOT NULL,
                taken_at TEXT, taken_at_override TEXT, created_at TEXT NOT NULL,
                is_favorite INTEGER NOT NULL DEFAULT 0, is_trashed INTEGER NOT NULL DEFAULT 0,
                aesthetic_score REAL, blur_score REAL, nsfw_score REAL);
             CREATE TABLE tags (media_id TEXT, tag TEXT, confidence REAL);
             CREATE TABLE captions (media_id TEXT, text TEXT);
             CREATE TABLE faces (media_id TEXT, person_id TEXT);
             CREATE TABLE exif_data (media_id TEXT, camera_model TEXT, gps_lat REAL, gps_lon REAL);
             CREATE TABLE geocode_cache (lat REAL, lon REAL, name TEXT);

             INSERT INTO media_items (id, filename, media_type, created_at, aesthetic_score)
             VALUES ('sharp', 'IMG_100%.jpg', 'image', '2024-06-01', 8.0),
                    ('blurry', 'IMG_2.jpg', 'image', '2024-06-02', 3.0),
                    ('mountain', 'IMG_3.jpg', 'image', '2024-06-03', NULL);
             INSERT INTO tags (media_id, tag) VALUES
                    ('sharp', 'beach'), ('blurry', 'beach'), ('mountain', 'mountain');
             INSERT INTO exif_data (media_id, camera_model, gps_lat, gps_lon)
             VALUES ('sharp', 'X100V', 48.8566, 2.3522);
             INSERT INTO geocode_cache (lat, lon, name) VALUES (48.86, 2.35, 'Paris, France');",
        )
        .unwrap();
        conn
    }

    fn run(conn: &Connection, rules: &[SmartAlbumRule], match_type: &str) -> Vec<String> {
        let mut ids = query_smart_album(conn, rules, match_type, 100).unwrap();
        ids.sort();
        ids
    }

    #[test]
    fn an_unknown_rule_is_refused_rather_than_dropped() {
        let conn = memory_db();
        // The old parser skipped this rule, so the album silently returned every
        // beach photo instead of none.
        let rules = vec![rule("tag", "is", "beach"), rule("altitude", "is", "high")];
        let error = query_smart_album(&conn, &rules, MATCH_ALL, 100).unwrap_err();

        assert!(error.contains("altitude"), "{error}");
        assert!(error.contains("accepted:"), "{error}");
    }

    #[test]
    fn an_operator_the_kind_does_not_accept_names_the_alternatives() {
        let conn = memory_db();
        let error = query_smart_album(&conn, &[rule("person", "contains", "x")], MATCH_ALL, 100)
            .unwrap_err();

        assert!(error.contains("\"person\""), "{error}");
        assert!(error.contains("is"), "{error}");
    }

    #[test]
    fn all_intersects_the_rules_and_any_unions_them() {
        let conn = memory_db();
        let rules = vec![rule("tag", "is", "beach"), rule("tag", "is", "mountain")];

        // No photo carries both tags — the case that used to produce a puzzling
        // empty album with no way to ask for "either".
        assert!(run(&conn, &rules, MATCH_ALL).is_empty());
        assert_eq!(run(&conn, &rules, MATCH_ANY), ["blurry", "mountain", "sharp"]);
    }

    #[test]
    fn an_unknown_match_type_is_refused() {
        let conn = memory_db();
        let error =
            query_smart_album(&conn, &[rule("tag", "is", "beach")], "maybe", 100).unwrap_err();

        assert!(error.contains("maybe"), "{error}");
    }

    #[test]
    fn negation_keeps_the_photos_that_were_never_scored() {
        let conn = memory_db();
        // "mountain" has no aesthetic score at all. Plain SQL would drop it from
        // both `>= 7` and `NOT (>= 7)`, which reads as a bug to anyone looking.
        assert_eq!(run(&conn, &[rule("aesthetic", "at_least", "7")], MATCH_ALL), ["sharp"]);
        assert_eq!(
            run(&conn, &[rule("aesthetic", "not_at_least", "7")], MATCH_ALL),
            ["blurry", "mountain"]
        );
    }

    #[test]
    fn a_score_rule_needs_an_actual_number() {
        let conn = memory_db();
        let error = query_smart_album(&conn, &[rule("blur", "at_most", "low")], MATCH_ALL, 100)
            .unwrap_err();

        assert!(error.contains("number"), "{error}");
    }

    #[test]
    fn like_wildcards_in_the_value_are_matched_literally() {
        let conn = memory_db();
        // Only "IMG_100%.jpg" contains a literal '%'; an unescaped pattern would
        // have matched all three filenames.
        assert_eq!(run(&conn, &[rule("filename", "contains", "%")], MATCH_ALL), ["sharp"]);
    }

    #[test]
    fn a_place_rule_matches_through_the_rounded_geocode_cache() {
        let conn = memory_db();
        assert_eq!(run(&conn, &[rule("place", "contains", "Paris")], MATCH_ALL), ["sharp"]);
        assert!(run(&conn, &[rule("place", "is", "Paris")], MATCH_ALL).is_empty());
        assert_eq!(
            run(&conn, &[rule("place", "is", "Paris, France")], MATCH_ALL),
            ["sharp"]
        );
    }

    #[test]
    fn trashed_photos_stay_out_of_an_any_album() {
        let conn = memory_db();
        conn.execute("UPDATE media_items SET is_trashed = 1 WHERE id = 'sharp'", [])
            .unwrap();
        let rules = vec![rule("tag", "is", "beach"), rule("tag", "is", "mountain")];

        assert_eq!(run(&conn, &rules, MATCH_ANY), ["blurry", "mountain"]);
    }
}
