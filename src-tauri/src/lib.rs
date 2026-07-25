mod ai;
mod commands;
mod db;
mod indexing;
mod jobs;
mod models;
mod state;
mod thumbnails;
mod watcher;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("hive.db");
            let conn = db::open(&db_path)?;

            let existing_folders: Vec<(String, String)> = {
                let mut stmt = conn.prepare(
                    "SELECT id, path FROM folders WHERE is_watched = 1",
                )?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };

            let ai_state = std::sync::Arc::new(ai::AiState::default());

            let watchers = watcher::WatcherRegistry::default();
            let handle = app.handle().clone();
            for (folder_id, folder_path) in existing_folders {
                let _ = watchers.watch(
                    handle.clone(),
                    db_path.clone(),
                    app_data_dir.clone(),
                    folder_id,
                    std::path::PathBuf::from(folder_path),
                    ai_state.clone(),
                );
            }

            // Warm-load the CLIP model in the background if it was already downloaded in a
            // previous session, so semantic search and embed-as-you-go work immediately.
            if ai::model_manager::clip_models_ready(&app_data_dir) {
                let ai_state = ai_state.clone();
                let clip_dir = ai::model_manager::clip_dir(&app_data_dir);
                tauri::async_runtime::spawn_blocking(move || {
                    if let Ok(model) = ai::clip::ClipModel::load(&clip_dir) {
                        *ai_state.clip.lock().unwrap() = Some(model);
                    }
                });
            }

            app.manage(AppState {
                db_path,
                app_data_dir,
                conn: std::sync::Mutex::new(conn),
                watchers,
                cancelled_jobs: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
                ai: ai_state,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folders::list_folders,
            commands::folders::list_folders_with_stats,
            commands::folders::add_watched_folder,
            commands::folders::remove_watched_folder,
            commands::media::scan_folder,
            commands::media::cancel_job,
            commands::media::search_media,
            commands::media::get_media_page,
            commands::media::get_media_detail,
            commands::media::read_media_bytes,
            commands::media::set_favorite,
            commands::media::set_trashed,
            commands::media::get_trash,
            commands::media::delete_media_permanently,
            commands::media::get_places,
            commands::media::get_library_stats,
            commands::ai::get_ai_status,
            commands::ai::download_ai_models,
            commands::ai::semantic_search,
            commands::ai::backfill_embeddings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hive");
}
