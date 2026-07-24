use crate::watcher::WatcherRegistry;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub db_path: PathBuf,
    pub app_data_dir: PathBuf,
    pub conn: Mutex<Connection>,
    pub watchers: WatcherRegistry,
    pub cancelled_jobs: Arc<Mutex<HashSet<String>>>,
}
