use crate::indexing;
use crate::models::ExplorerEntry;
use crate::state::AppState;
use rusqlite::params;
use std::path::{Path, PathBuf};
use tauri::State;

/// Root entries of the browser: every drive Windows currently exposes, plus the
/// folders Hive already watches so they are one click away.
#[tauri::command]
pub fn list_drives(state: State<'_, AppState>) -> Result<Vec<ExplorerEntry>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut entries = Vec::new();

    #[cfg(windows)]
    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        if !Path::new(&root).is_dir() {
            continue;
        }
        entries.push(ExplorerEntry {
            name: format!("{}:", letter as char),
            path: root,
            is_directory: true,
            media_count: 0,
            indexed_count: 0,
            is_watched: false,
        });
    }

    #[cfg(not(windows))]
    entries.push(ExplorerEntry {
        name: "/".into(),
        path: "/".into(),
        is_directory: true,
        media_count: 0,
        indexed_count: 0,
        is_watched: false,
    });

    // Mark the drives that already hold a watched folder.
    let watched: Vec<String> = conn
        .prepare("SELECT path FROM folders")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|path| path.ok())
        .collect();

    for entry in &mut entries {
        entry.is_watched = watched
            .iter()
            .any(|path| path.to_lowercase().starts_with(&entry.path.to_lowercase()));
    }

    Ok(entries)
}

/// Lists one directory: sub-directories first, then how many supported media
/// files sit directly inside, and how many of those Hive has already indexed.
#[tauri::command]
pub fn list_directory(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<ExplorerEntry>, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("{path} is not a folder"));
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let watched: Vec<String> = conn
        .prepare("SELECT path FROM folders")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|path| path.ok())
        .collect();

    let readable = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    let mut directories = Vec::new();
    let mut media_here = 0i64;

    for entry in readable.filter_map(|entry| entry.ok()) {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Hidden and system folders are noise in a photo browser.
        if name.starts_with('.') || name.starts_with('$') {
            continue;
        }

        if entry_path.is_dir() {
            let path_string = entry_path.to_string_lossy().to_string();
            let indexed_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM media_items WHERE path LIKE ?1 || '%' AND is_trashed = 0",
                    params![format!("{path_string}\\")],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            directories.push(ExplorerEntry {
                is_watched: watched.iter().any(|w| w.eq_ignore_ascii_case(&path_string)),
                name,
                path: path_string,
                is_directory: true,
                media_count: count_media_in(&entry_path),
                indexed_count,
            });
        } else if indexing::is_supported_media(&entry_path).is_some() {
            media_here += 1;
        }
    }

    directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // The current folder itself is reported as the first row so the UI can show
    // "42 photos here" without a second call.
    let self_path = dir.to_string_lossy().to_string();
    let self_indexed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM media_items WHERE path LIKE ?1 || '%' AND is_trashed = 0",
            params![format!("{self_path}\\")],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut entries = vec![ExplorerEntry {
        name: ".".into(),
        is_watched: watched.iter().any(|w| w.eq_ignore_ascii_case(&self_path)),
        path: self_path,
        is_directory: true,
        media_count: media_here,
        indexed_count: self_indexed,
    }];
    entries.extend(directories);
    Ok(entries)
}

/// Counts supported files one level deep only. A recursive count would stall the
/// browser on folders like C:\ that hold hundreds of thousands of files.
fn count_media_in(dir: &Path) -> i64 {
    let Ok(readable) = std::fs::read_dir(dir) else {
        return 0;
    };
    readable
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter(|entry| indexing::is_supported_media(&entry.path()).is_some())
        .count() as i64
}

/// The parent of a path, or None when already at a drive root.
#[tauri::command]
pub fn parent_directory(path: String) -> Option<String> {
    Path::new(&path)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .filter(|parent| !parent.is_empty())
}
