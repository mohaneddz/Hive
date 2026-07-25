mod ai;
mod commands;
mod db;
mod duplicates;
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

            // A restore staged during the previous session can only be applied
            // here, before anything opens the database.
            commands::backup::take_pending_restore(&app_data_dir, &db_path)?;

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

            // Warm-load the CLIP/OCR models in the background if they were already downloaded
            // in a previous session, so semantic search / OCR-as-you-go work immediately.
            if ai::model_manager::clip_models_ready(&app_data_dir) {
                let ai_state = ai_state.clone();
                let clip_dir = ai::model_manager::clip_dir(&app_data_dir);
                tauri::async_runtime::spawn_blocking(move || {
                    if let Ok(model) = ai::clip::ClipModel::load(&clip_dir) {
                        *ai_state.clip.lock().unwrap() = Some(model);
                    }
                });
            }
            if ai::model_manager::ocr_models_ready(&app_data_dir) {
                let ai_state = ai_state.clone();
                let ocr_dir = ai::model_manager::ocr_dir(&app_data_dir);
                tauri::async_runtime::spawn_blocking(move || {
                    if let Ok(model) = ai::ocr::OcrModel::load(&ocr_dir) {
                        *ai_state.ocr.lock().unwrap() = Some(model);
                    }
                });
            }
            if ai::model_manager::face_models_ready(&app_data_dir) {
                let ai_state = ai_state.clone();
                let face_dir = ai::model_manager::face_dir(&app_data_dir);
                tauri::async_runtime::spawn_blocking(move || {
                    if let Ok(model) = ai::face::FaceModel::load(&face_dir) {
                        *ai_state.face.lock().unwrap() = Some(model);
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
            commands::media::delete_media_permanently,
            commands::media::get_places,
            commands::media::get_library_stats,
            commands::media::backfill_thumbnails,
            commands::ai::get_ai_status,
            commands::ai::download_ai_models,
            commands::ai::semantic_search,
            commands::ai::backfill_embeddings,
            commands::ai::download_ocr_models,
            commands::ai::backfill_ocr,
            commands::duplicates::scan_duplicates,
            commands::duplicates::get_duplicate_groups,
            commands::duplicates::dismiss_duplicate_group,
            commands::faces::download_face_models,
            commands::faces::backfill_faces,
            commands::faces::list_people,
            commands::faces::rename_person,
            commands::faces::merge_people,
            commands::faces::get_person_media,
            commands::faces::read_face_crop_bytes,
            commands::media::set_hidden,
            commands::media::set_archived,
            commands::media::touch_last_viewed,
            commands::media::empty_trash,
            commands::media::get_on_this_day,
            commands::folders::set_folder_watched,
            commands::albums::list_albums,
            commands::albums::get_album,
            commands::albums::create_album,
            commands::albums::update_album,
            commands::albums::delete_album,
            commands::albums::set_album_cover,
            commands::albums::add_media_to_album,
            commands::albums::remove_media_from_album,
            commands::albums::list_albums_for_media,
            commands::places::list_places,
            commands::places::list_media_at_place,
            commands::explorer::list_drives,
            commands::explorer::list_directory,
            commands::explorer::parent_directory,
            commands::editor::apply_edits,
            commands::editor::update_media_metadata,
            commands::batch::preview_batch_rename,
            commands::batch::batch_rename,
            commands::batch::compress_images,
            commands::batch::convert_images,
            commands::backup::backup_library,
            commands::backup::inspect_backup,
            commands::backup::restore_library,
            commands::backup::cancel_pending_restore,
            commands::backup::has_pending_restore,
            commands::utilities::scan_library_health,
            commands::utilities::remove_missing_entries,
            commands::utilities::get_storage_stats,
            commands::utilities::clear_thumbnail_cache,
            commands::utilities::export_media,
            commands::organize::get_timeline,
            commands::organize::list_media_in_bucket,
            commands::organize::detect_events,
            commands::organize::detect_trips,
            commands::quality::scan_blur,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hive");
}
