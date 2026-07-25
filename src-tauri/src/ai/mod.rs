pub mod clip;
pub mod model_manager;
pub mod ocr;

use std::sync::Mutex;

use clip::ClipModel;
use ocr::OcrModel;

#[derive(Default)]
pub struct AiState {
    pub clip: Mutex<Option<ClipModel>>,
    pub ocr: Mutex<Option<OcrModel>>,
}
