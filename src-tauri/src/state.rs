use crate::watcher::WatcherRegistry;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub db_path: PathBuf,
    pub app_data_dir: PathBuf,
    pub conn: Mutex<Connection>,
    pub watchers: WatcherRegistry,
}
