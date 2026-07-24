use crate::models::JobProgress;
use rusqlite::{params, Connection};
use tauri::{AppHandle, Emitter};

pub const EVENT_JOB_PROGRESS: &str = "job:progress";

pub fn create_job(conn: &Connection, kind: &str) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO jobs (id, kind, status, payload, error, created_at, updated_at)
         VALUES (?1, ?2, 'running', NULL, NULL, ?3, ?3)",
        params![id, kind, now],
    )?;
    Ok(id)
}

pub fn emit_progress(
    app: &AppHandle,
    conn: &Connection,
    job_id: &str,
    kind: &str,
    status: &str,
    current: i64,
    total: i64,
    message: Option<String>,
) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, job_id],
    );
    let _ = app.emit(
        EVENT_JOB_PROGRESS,
        JobProgress {
            id: job_id.to_string(),
            kind: kind.to_string(),
            status: status.to_string(),
            current,
            total,
            message,
        },
    );
}

pub fn fail_job(conn: &Connection, job_id: &str, error: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE jobs SET status = 'failed', error = ?1, updated_at = ?2 WHERE id = ?3",
        params![error, now, job_id],
    );
}
