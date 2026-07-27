pub mod aesthetic;
pub mod aesthetic_weights;
pub mod captioning;
pub mod clip;
pub mod face;
pub mod model_manager;
pub mod nsfw;
pub mod ocr;

use std::sync::Mutex;

use captioning::CaptionModel;
use clip::ClipModel;
use face::FaceModel;
use nsfw::NsfwModel;
use ocr::OcrModel;

/// Models held in memory once loaded.
///
/// Aesthetic scoring is absent on purpose: it is a linear layer over a CLIP
/// embedding, so there is no session to keep alive.
#[derive(Default)]
pub struct AiState {
    pub clip: Mutex<Option<ClipModel>>,
    pub ocr: Mutex<Option<OcrModel>>,
    pub face: Mutex<Option<FaceModel>>,
    pub nsfw: Mutex<Option<NsfwModel>>,
    pub caption: Mutex<Option<CaptionModel>>,
}
