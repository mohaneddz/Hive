pub mod aesthetic;
pub mod aesthetic_weights;
pub mod captioning;
pub mod clip;
pub mod face;
pub mod llm;
pub mod model_manager;
pub mod nsfw;
pub mod ocr;

use std::sync::{atomic::{AtomicU64, Ordering}, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use captioning::CaptionModel;
use clip::ClipModel;
use face::FaceModel;
use llm::ChatModel;
use nsfw::NsfwModel;
use ocr::OcrModel;

/// Models held in memory once loaded.
///
/// Aesthetic scoring is absent on purpose: it is a linear layer over a CLIP
/// embedding, so there is no session to keep alive.
pub struct AiState {
    pub clip: Mutex<Option<ClipModel>>,
    pub ocr: Mutex<Option<OcrModel>>,
    pub face: Mutex<Option<FaceModel>>,
    pub llm: Mutex<Option<ChatModel>>,
    pub nsfw: Mutex<Option<NsfwModel>>,
    pub caption: Mutex<Option<CaptionModel>>,
    last_used_at: AtomicU64,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            clip: Mutex::new(None),
            ocr: Mutex::new(None),
            face: Mutex::new(None),
            llm: Mutex::new(None),
            nsfw: Mutex::new(None),
            caption: Mutex::new(None),
            last_used_at: AtomicU64::new(0),
        }
    }
}

impl AiState {
    pub fn touch(&self) {
        self.last_used_at.store(now_seconds(), Ordering::Relaxed);
    }

    pub fn unload_all(&self) -> i64 {
        let mut unloaded = 0;
        macro_rules! unload {
            ($slot:expr) => {
                if $slot.lock().ok().and_then(|mut model| model.take()).is_some() {
                    unloaded += 1;
                }
            };
        }
        unload!(self.clip);
        unload!(self.ocr);
        unload!(self.face);
        unload!(self.llm);
        unload!(self.nsfw);
        unload!(self.caption);
        unloaded
    }

    pub fn unload_if_idle(&self, idle_seconds: u64) -> i64 {
        let last_used = self.last_used_at.load(Ordering::Relaxed);
        if last_used == 0 || now_seconds().saturating_sub(last_used) < idle_seconds {
            return 0;
        }
        let unloaded = self.unload_all();
        if unloaded > 0 {
            self.last_used_at.store(0, Ordering::Relaxed);
        }
        unloaded
    }
}

fn now_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
