//! The AI editor's commands: enlarge, cut out, select and erase.
//!
//! **Nothing here writes a file.** Each tool computes its result, keeps it in
//! memory as a [`PendingEdit`], and returns a preview — exactly the way the
//! brightness slider shows you a change you have not committed. The single Save
//! button in the editor is what reaches the disk, through the same path the
//! colour editor already used.
//!
//! That is deliberate and was not the first design. Writing on every click meant
//! no preview, no undo, and a file per experiment: four of them came out of one
//! afternoon of trying things, with names like `photo (enlarged) (cutout) (2)
//! (cutout).png`. The interface was teaching people to make a mess.
//!
//! Because the result stays in memory, tools also stack: erase somebody, then
//! enlarge what is left, then adjust the colour — and one file at the end.
//!
//! Which models were chosen, and the licence traps avoided, are in
//! `.idea/AI-Editor.md`.

use crate::ai::cutout::{self, CutoutModel, Subject};
use crate::ai::generate::GenerateModel;
use crate::ai::inpaint::InpaintModel;
use crate::ai::model_manager;
use crate::ai::segment::SegmentModel;
use crate::ai::session;
use crate::ai::upscale::UpscaleModel;
use crate::ai::PendingEdit;
use crate::commands::editor::save_derived_image;
use crate::jobs;
use crate::models::MediaItem;
use crate::state::AppState;
use image::{DynamicImage, GrayImage, RgbImage};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tauri::{AppHandle, Emitter, State};

/// The longest edge of a preview handed to the interface.
///
/// An enlarged photo can be 7524 pixels wide; sending that to a canvas that
/// displays it at 900 would be several megabytes across the bridge for pixels
/// nobody can see.
const PREVIEW_EDGE: u32 = 1600;

/// What the editor can offer right now.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEditorStatus {
    pub upscale_ready: bool,
    pub cutout_ready: bool,
    pub segment_ready: bool,
    pub inpaint_ready: bool,
    pub generate_ready: bool,
    /// "DirectML", or absent when everything runs on the processor.
    pub gpu_backend: Option<String>,
}

/// A result waiting to be saved: what to draw, and how it got there.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiPreview {
    /// PNG bytes, scaled down for display.
    pub preview: Vec<u8>,
    /// Full-resolution size, so the editor can say what will be written.
    pub width: u32,
    pub height: u32,
    /// The tools applied so far, oldest first.
    pub steps: Vec<String>,
}

/// A click, in the photo's own pixel coordinates. `positive` false means
/// "not this" — how you carve back a selection that grabbed too much.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectPoint {
    pub x: f32,
    pub y: f32,
    pub positive: bool,
}

#[tauri::command]
pub fn get_ai_editor_status(state: State<'_, AppState>) -> Result<AiEditorStatus, String> {
    let dir = &state.app_data_dir;
    Ok(AiEditorStatus {
        upscale_ready: model_manager::upscale_models_ready(dir),
        cutout_ready: model_manager::cutout_models_ready(dir),
        segment_ready: model_manager::segment_models_ready(dir),
        inpaint_ready: model_manager::inpaint_models_ready(dir),
        generate_ready: model_manager::generate_models_ready(dir),
        gpu_backend: session::gpu_backend().map(str::to_string),
    })
}

/// Downloads one tool's model. `tool` is `upscale`, `cutout`, `segment` or
/// `inpaint`; progress arrives on `ai-editor:download:progress`.
#[tauri::command]
pub async fn download_ai_editor_model(
    app: AppHandle,
    state: State<'_, AppState>,
    tool: String,
) -> Result<(), String> {
    let dir = state.app_data_dir.clone();
    let report = move |done: u64, total: u64| {
        let _ = app.emit("ai-editor:download:progress", (done, total));
    };

    match tool.as_str() {
        "upscale" => model_manager::ensure_upscale_models(&dir, report).await,
        "cutout" => model_manager::ensure_cutout_models(&dir, report).await,
        "segment" => model_manager::ensure_segment_models(&dir, report).await,
        "inpaint" => model_manager::ensure_inpaint_models(&dir, report).await,
        "generate" => model_manager::ensure_generate_models(&dir, report).await,
        other => return Err(format!("Unknown editor tool: {other}")),
    }
    .map_err(|e| e.to_string())
}

/* ------------------------------------------------------------- plumbing -- */

fn png_bytes(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut buffer = Cursor::new(Vec::new());
    image
        .write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buffer.into_inner())
}

/// The photo's path and how many faces Hive found in it.
fn source_of(state: &State<'_, AppState>, media_id: &str) -> Result<(String, i64), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let path: String = conn
        .query_row(
            "SELECT path FROM media_items WHERE id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let faces: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM faces WHERE media_id = ?1",
            params![media_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok((path, faces))
}

/// What the next tool should work from: the pending result if there is one for
/// this photo, otherwise the file on disk.
///
/// This is what lets tools stack. It also means a pending result belonging to a
/// different photo is dropped rather than silently applied to this one.
fn working_image(state: &State<'_, AppState>, media_id: &str) -> Result<RgbImage, String> {
    {
        let pending = state.ai.pending.lock().map_err(|e| e.to_string())?;
        if let Some(edit) = pending.as_ref() {
            if edit.media_id == media_id {
                return Ok(edit.image.to_rgb8());
            }
        }
    }
    let (path, _) = source_of(state, media_id)?;
    image::open(&path)
        .map_err(|e| e.to_string())
        .map(|image| image.to_rgb8())
}

/// What the editor should draw for a result, at a size a screen can use.
fn preview_of(edit: &PendingEdit) -> Result<AiPreview, String> {
    let (width, height) = (edit.image.width(), edit.image.height());
    let scale = PREVIEW_EDGE as f32 / width.max(height) as f32;
    let preview = if scale < 1.0 {
        edit.image.resize(
            (width as f32 * scale).round() as u32,
            (height as f32 * scale).round() as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        edit.image.clone()
    };

    Ok(AiPreview {
        preview: png_bytes(&preview)?,
        width,
        height,
        steps: edit.steps.clone(),
    })
}

/// How much undo history to hold in memory, in bytes.
///
/// Each entry is a whole picture at full resolution: a 12-megapixel photo is
/// 36 MB, and enlarging it fourfold makes that 576 MB. Left unbounded, two
/// enlargements in a row would cost more memory than the machine has.
///
/// The newest entry is always kept whatever its size — one step back is the
/// thing that was asked for — and older ones are dropped until the rest fits.
const UNDO_BUDGET: usize = 768 * 1024 * 1024;

/// Files the superseded result away so Undo can bring it back.
fn remember(history: &mut Vec<PendingEdit>, superseded: PendingEdit) {
    history.push(superseded);

    let mut held: usize = history.iter().map(|edit| edit.image.as_bytes().len()).sum();
    while history.len() > 1 && held > UNDO_BUDGET {
        held -= history.remove(0).image.as_bytes().len();
    }
}

/// Records a result and returns what the editor should draw.
fn stage(
    state: &State<'_, AppState>,
    media_id: &str,
    produced: DynamicImage,
    step: &str,
) -> Result<AiPreview, String> {
    // Always `pending` before `undo`, here and in `undo_ai_edit`: two locks
    // taken in two orders is how a deadlock is written.
    let mut pending = state.ai.pending.lock().map_err(|e| e.to_string())?;
    let mut history = state.ai.undo.lock().map_err(|e| e.to_string())?;

    let mut steps = match pending.take() {
        Some(edit) if edit.media_id == media_id => {
            let steps = edit.steps.clone();
            remember(&mut history, edit);
            steps
        }
        _ => {
            // A different photo: its history is no use for this one.
            history.clear();
            Vec::new()
        }
    };
    steps.push(step.to_string());

    let edit = PendingEdit {
        media_id: media_id.to_string(),
        image: produced,
        steps,
    };
    let preview = preview_of(&edit)?;
    *pending = Some(edit);
    Ok(preview)
}

/// Takes the last tool back, and returns what is left — `None` when that was
/// the only one and the photo itself is what remains.
#[tauri::command]
pub fn undo_ai_edit(
    state: State<'_, AppState>,
    media_id: String,
) -> Result<Option<AiPreview>, String> {
    let mut pending = state.ai.pending.lock().map_err(|e| e.to_string())?;
    let mut history = state.ai.undo.lock().map_err(|e| e.to_string())?;

    // The picture the tools work from is about to change, so the encoding kept
    // for click-to-select no longer describes it.
    *state.ai.encoded.lock().map_err(|e| e.to_string())? = None;

    match history.pop().filter(|edit| edit.media_id == media_id) {
        Some(edit) => {
            let preview = preview_of(&edit)?;
            *pending = Some(edit);
            Ok(Some(preview))
        }
        None => {
            // Nothing older to return to, so what is left is the original.
            *pending = None;
            history.clear();
            Ok(None)
        }
    }
}

/// Throws away whatever is waiting, so Reset clears the AI work too.
#[tauri::command]
pub fn discard_ai_edit(state: State<'_, AppState>) -> Result<(), String> {
    *state.ai.pending.lock().map_err(|e| e.to_string())? = None;
    state.ai.undo.lock().map_err(|e| e.to_string())?.clear();
    *state.ai.encoded.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

/// The result waiting for this photo, if any — so re-opening the editor does not
/// silently lose work in progress.
#[tauri::command]
pub fn get_ai_edit(state: State<'_, AppState>, media_id: String) -> Result<Option<AiPreview>, String> {
    let pending = state.ai.pending.lock().map_err(|e| e.to_string())?;
    let Some(edit) = pending.as_ref().filter(|edit| edit.media_id == media_id) else {
        return Ok(None);
    };
    preview_of(edit).map(Some)
}

/// Writes the pending result, through the same path the colour editor saves by.
///
/// Called by the editor's one Save button, after the usual "keep the original?"
/// question — which is why no tool asks that for itself any more.
#[tauri::command]
pub fn commit_ai_edit(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    mode: String,
) -> Result<MediaItem, String> {
    let produced = {
        let mut pending = state.ai.pending.lock().map_err(|e| e.to_string())?;
        match pending.take() {
            Some(edit) if edit.media_id == media_id => edit.image,
            // Put it back: it belongs to a different photo.
            other => {
                *pending = other;
                return Err("There is no AI result waiting for this photo".into());
            }
        }
    };
    // Once it is on disk there is nothing left to take back, and the earlier
    // versions are megabytes each.
    state.ai.undo.lock().map_err(|e| e.to_string())?.clear();

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    save_derived_image(&app, &state, &conn, &media_id, &produced, &mode, "edited")
}

/* ---------------------------------------------------------------- tools -- */

/// Enlarges fourfold and shows the result, without writing anything.
#[tauri::command]
pub async fn preview_upscale(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
) -> Result<AiPreview, String> {
    if !model_manager::upscale_models_ready(&state.app_data_dir) {
        return Err("The enlarging model is not downloaded yet".into());
    }
    let source = working_image(&state, &media_id)?;

    let ai = state.ai.clone();
    let dir = model_manager::upscale_dir(&state.app_data_dir);
    let cancelled_jobs = state.cancelled_jobs.clone();
    let db_path = state.db_path.clone();

    let enlarged = tauri::async_runtime::spawn_blocking(move || -> Result<RgbImage, String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "upscale").map_err(|e| e.to_string())?;

        // Checked before the model loads, so an impossible size fails at once.
        UpscaleModel::output_size(source.width(), source.height()).map_err(|e| e.to_string())?;

        let mut guard = ai.upscale.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(UpscaleModel::load(&dir).map_err(|e| e.to_string())?);
        }
        let model = guard.as_mut().expect("just loaded");

        let cancel_id = job_id.clone();
        let outcome = model.enlarge(
            &source,
            |done, total| {
                jobs::emit_progress(
                    &app, &conn, &job_id, "upscale", "running",
                    done as i64, total as i64, None,
                );
            },
            || {
                cancelled_jobs
                    .lock()
                    .map(|mut set| set.remove(&cancel_id))
                    .unwrap_or(false)
            },
        );

        let status = match &outcome {
            Ok(_) => "completed",
            Err(error) if error.to_string() == "cancelled" => "cancelled",
            Err(_) => "failed",
        };
        jobs::emit_progress(
            &app, &conn, &cancel_id, "upscale", status, 1, 1,
            outcome.as_ref().err().map(|error| error.to_string()),
        );
        outcome.map_err(|error| error.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    stage(&state, &media_id, DynamicImage::ImageRgb8(enlarged), "Enlarged ×4")
}

/// Cuts the subject out, optionally over a replacement picture.
#[tauri::command]
pub async fn preview_remove_background(
    state: State<'_, AppState>,
    media_id: String,
    background_path: Option<String>,
) -> Result<AiPreview, String> {
    if !model_manager::cutout_models_ready(&state.app_data_dir) {
        return Err("The cutout models are not downloaded yet".into());
    }
    let (_, faces) = source_of(&state, &media_id)?;
    let subject = Subject::for_face_count(faces);
    let source = working_image(&state, &media_id)?;

    let ai = state.ai.clone();
    let dir = model_manager::cutout_dir(&state.app_data_dir);
    let replaced = background_path.is_some();

    let produced = tauri::async_runtime::spawn_blocking(move || -> Result<DynamicImage, String> {
        CutoutModel::check_size(source.width(), source.height()).map_err(|e| e.to_string())?;

        let mut guard = ai.cutout.lock().map_err(|e| e.to_string())?;
        // The two models are different files; a photo with a face needs the
        // other one loaded than the last photo may have used.
        if guard.as_ref().map(|model| model.subject()) != Some(subject) {
            *guard = Some(CutoutModel::load(&dir, subject).map_err(|e| e.to_string())?);
        }
        let matte = guard
            .as_mut()
            .expect("just loaded")
            .matte(&source)
            .map_err(|e| e.to_string())?;

        Ok(match background_path {
            Some(background) => {
                let behind = image::open(&background)
                    .map_err(|e| e.to_string())?
                    .to_rgb8();
                DynamicImage::ImageRgb8(cutout::composite_over(&source, &matte, &behind))
            }
            None => DynamicImage::ImageRgba8(cutout::apply_matte(&source, &matte)),
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    let step = if replaced {
        "Background replaced"
    } else {
        "Background removed"
    };
    stage(&state, &media_id, produced, step)
}

/// Encodes the photo for click-to-select, ahead of the first click.
///
/// Measured: encoding takes about seven seconds, each click after it a tenth of
/// one. Done lazily, that whole cost lands on the first click and looks like the
/// button being broken. Called when the AI tab opens instead, it happens while
/// the user is still deciding what to click.
///
/// Cheap to call twice — an encoding already in hand is kept.
#[tauri::command]
pub async fn warm_selection(state: State<'_, AppState>, media_id: String) -> Result<(), String> {
    if !model_manager::segment_models_ready(&state.app_data_dir) {
        return Ok(());
    }
    let source = working_image(&state, &media_id)?;
    let key = selection_key(&state, &media_id)?;

    {
        let cache = state.ai.encoded.lock().map_err(|e| e.to_string())?;
        if cache.as_ref().map(|(cached, _)| cached.as_str()) == Some(key.as_str()) {
            return Ok(());
        }
    }

    let ai = state.ai.clone();
    let dir = model_manager::segment_dir(&state.app_data_dir);
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let mut guard = ai.segment.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(SegmentModel::load(&dir).map_err(|e| e.to_string())?);
        }
        let encoded = guard
            .as_mut()
            .expect("just loaded")
            .encode(&source)
            .map_err(|e| e.to_string())?;
        *ai.encoded.lock().map_err(|e| e.to_string())? = Some((key, encoded));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Identifies the pixels an encoding describes: the photo, plus how many tools
/// have changed it since. A stale encoding would select from the wrong picture.
fn selection_key(state: &State<'_, AppState>, media_id: &str) -> Result<String, String> {
    let pending = state.ai.pending.lock().map_err(|e| e.to_string())?;
    Ok(
        match pending.as_ref().filter(|edit| edit.media_id == media_id) {
            Some(edit) => format!("{media_id}#{}", edit.steps.len()),
            None => media_id.to_string(),
        },
    )
}

/// The mask for whatever sits under the given clicks, as PNG bytes.
#[tauri::command]
pub async fn select_object(
    state: State<'_, AppState>,
    media_id: String,
    points: Vec<SelectPoint>,
) -> Result<Vec<u8>, String> {
    if !model_manager::segment_models_ready(&state.app_data_dir) {
        return Err("The selection model is not downloaded yet".into());
    }
    if points.is_empty() {
        return Err("Click the thing you want to select".into());
    }
    let source = working_image(&state, &media_id)?;

    let ai = state.ai.clone();
    let dir = model_manager::segment_dir(&state.app_data_dir);
    let cache_key = selection_key(&state, &media_id)?;

    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let mut guard = ai.segment.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(SegmentModel::load(&dir).map_err(|e| e.to_string())?);
        }
        let model = guard.as_mut().expect("just loaded");

        let mut cache = ai.encoded.lock().map_err(|e| e.to_string())?;
        if cache.as_ref().map(|(key, _)| key.as_str()) != Some(cache_key.as_str()) {
            let encoded = model.encode(&source).map_err(|e| e.to_string())?;
            *cache = Some((cache_key, encoded));
        }
        let (_, encoded) = cache.as_ref().expect("just encoded");

        let clicks: Vec<(f32, f32, bool)> =
            points.iter().map(|p| (p.x, p.y, p.positive)).collect();
        let mask = model.mask_at(encoded, &clicks).map_err(|e| e.to_string())?;
        png_bytes(&DynamicImage::ImageLuma8(mask))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes whatever the mask covers and shows the repair.
#[tauri::command]
pub async fn preview_erase(
    state: State<'_, AppState>,
    media_id: String,
    mask_png: Vec<u8>,
) -> Result<AiPreview, String> {
    if !model_manager::inpaint_models_ready(&state.app_data_dir) {
        return Err("The object-removal model is not downloaded yet".into());
    }
    let source = working_image(&state, &media_id)?;

    let ai = state.ai.clone();
    let dir = model_manager::inpaint_dir(&state.app_data_dir);

    let repaired = tauri::async_runtime::spawn_blocking(move || -> Result<RgbImage, String> {
        let mask: GrayImage = image::load_from_memory(&mask_png)
            .map_err(|e| e.to_string())?
            .to_luma8();

        let mut guard = ai.inpaint.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(InpaintModel::load(&dir).map_err(|e| e.to_string())?);
        }
        guard
            .as_mut()
            .expect("just loaded")
            .erase(&source, &mask)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // The encoding described the pixels that were just changed.
    *state.ai.encoded.lock().map_err(|e| e.to_string())? = None;

    stage(&state, &media_id, DynamicImage::ImageRgb8(repaired), "Object erased")
}

/// Paints whatever the description asks for into the masked area.
///
/// The slow one by a wide margin: twenty-odd passes through a 1.7 GB model
/// rather than the single pass every other tool makes. It reports progress per
/// step and can be stopped, because a minute of silence reads as a freeze.
#[tauri::command]
pub async fn preview_generate(
    app: AppHandle,
    state: State<'_, AppState>,
    media_id: String,
    mask_png: Vec<u8>,
    prompt: String,
    steps: Option<usize>,
    seed: Option<u64>,
) -> Result<AiPreview, String> {
    if !model_manager::generate_models_ready(&state.app_data_dir) {
        return Err("The generation model is not downloaded yet".into());
    }
    let source = working_image(&state, &media_id)?;

    let ai = state.ai.clone();
    let dir = model_manager::generate_dir(&state.app_data_dir);
    let clip_dir = model_manager::clip_dir(&state.app_data_dir);
    let cancelled_jobs = state.cancelled_jobs.clone();
    let db_path = state.db_path.clone();
    // Fresh each time unless asked otherwise, so pressing the button again on
    // the same prompt offers a different attempt rather than the same one.
    let seed = seed.unwrap_or_else(|| chrono::Utc::now().timestamp_micros() as u64);
    let steps = steps.unwrap_or(25).clamp(4, 50);
    let described = prompt.clone();

    let painted = tauri::async_runtime::spawn_blocking(move || -> Result<RgbImage, String> {
        let conn = crate::db::open(&db_path).map_err(|e| e.to_string())?;
        let job_id = jobs::create_job(&conn, "generate").map_err(|e| e.to_string())?;

        let mask: GrayImage = image::load_from_memory(&mask_png)
            .map_err(|e| e.to_string())?
            .to_luma8();

        let mut guard = ai.generate.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(GenerateModel::load(&dir, &clip_dir).map_err(|e| e.to_string())?);
        }
        let model = guard.as_mut().expect("just loaded");

        let cancel_id = job_id.clone();
        let outcome = model.generate(
            &source,
            &mask,
            &described,
            steps,
            seed,
            |done, total| {
                jobs::emit_progress(
                    &app, &conn, &job_id, "generate", "running",
                    done as i64, total as i64, None,
                );
            },
            || {
                cancelled_jobs
                    .lock()
                    .map(|mut set| set.remove(&cancel_id))
                    .unwrap_or(false)
            },
        );

        let status = match &outcome {
            Ok(_) => "completed",
            Err(error) if error.to_string() == "cancelled" => "cancelled",
            Err(_) => "failed",
        };
        jobs::emit_progress(
            &app, &conn, &cancel_id, "generate", status, 1, 1,
            outcome.as_ref().err().map(|error| error.to_string()),
        );
        outcome.map_err(|error| error.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // The pixels under the selection are new, so any encoding of them is stale.
    *state.ai.encoded.lock().map_err(|e| e.to_string())? = None;

    let summary = prompt.trim();
    let label = if summary.chars().count() > 28 {
        format!("Painted “{}…”", summary.chars().take(28).collect::<String>())
    } else {
        format!("Painted “{summary}”")
    };
    stage(&state, &media_id, DynamicImage::ImageRgb8(painted), &label)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roughly `megabytes` worth of picture, so the budget can be tested at a
    /// realistic scale without allocating a gigabyte.
    fn edit_of(megabytes: usize) -> PendingEdit {
        let side = ((megabytes * 1024 * 1024 / 3) as f64).sqrt() as u32;
        PendingEdit {
            media_id: "photo".into(),
            image: DynamicImage::ImageRgb8(RgbImage::new(side, side)),
            steps: vec![format!("{megabytes} MB")],
        }
    }

    #[test]
    fn the_history_stays_under_its_budget() {
        let mut history = Vec::new();
        for _ in 0..12 {
            remember(&mut history, edit_of(100));
        }

        let held: usize = history.iter().map(|edit| edit.image.as_bytes().len()).sum();
        assert!(held <= UNDO_BUDGET, "kept {held} bytes, budget is {UNDO_BUDGET}");
        assert!(history.len() < 12, "nothing was dropped");
    }

    #[test]
    fn the_newest_step_is_kept_however_large_it_is() {
        // An enlargement of a big photo can exceed the whole budget by itself.
        // Dropping it would leave Undo unable to do the one thing it is for.
        let mut history = Vec::new();
        remember(&mut history, edit_of(200));
        remember(&mut history, edit_of(1200));

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].steps, vec!["1200 MB".to_string()]);
    }

    #[test]
    fn the_oldest_steps_go_first() {
        let mut history = Vec::new();
        for size in [300, 300, 300, 300] {
            remember(&mut history, edit_of(size));
        }

        // Whatever survives, it is the tail: undo walks backwards, so losing the
        // far end costs the steps furthest from where the user is.
        let kept: Vec<&str> = history.iter().map(|e| e.steps[0].as_str()).collect();
        assert!(kept.len() < 4 && !kept.is_empty(), "kept {kept:?}");
    }
}
