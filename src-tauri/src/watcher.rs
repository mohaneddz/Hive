use crate::ai::AiState;
use crate::{db, indexing, thumbnails};
use notify::{RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub const EVENT_MEDIA_CHANGED: &str = "media:changed";

pub struct WatcherRegistry {
    watchers: Mutex<HashMap<String, notify::RecommendedWatcher>>,
}

impl Default for WatcherRegistry {
    fn default() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
        }
    }
}

impl WatcherRegistry {
    pub fn watch(
        &self,
        app: AppHandle,
        db_path: PathBuf,
        app_data_dir: PathBuf,
        folder_id: String,
        folder_path: PathBuf,
        ai: Arc<AiState>,
    ) -> anyhow::Result<()> {
        let fid = folder_id.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            if !matches!(
                event.kind,
                notify::EventKind::Create(_) | notify::EventKind::Modify(_)
            ) {
                return;
            }
            let Ok(conn) = db::open(&db_path) else { return };
            let mut changed = false;
            for path in &event.paths {
                if !path.is_file() || indexing::is_supported_media(path).is_none() {
                    continue;
                }
                if let Ok(Some(indexed)) = indexing::index_file(&conn, &fid, path) {
                    let _ = thumbnails::generate_for_image(
                        &conn,
                        &app_data_dir,
                        &indexed.item.id,
                        path,
                    );
                    crate::commands::ai::try_embed_image(&ai, &conn, &indexed.item.id, path);
                    changed = true;
                }
            }
            if changed {
                let _ = app.emit(EVENT_MEDIA_CHANGED, &fid);
            }
        })?;

        watcher.watch(&folder_path, RecursiveMode::Recursive)?;
        self.watchers.lock().unwrap().insert(folder_id, watcher);
        Ok(())
    }

    pub fn unwatch(&self, folder_id: &str) {
        self.watchers.lock().unwrap().remove(folder_id);
    }
}
