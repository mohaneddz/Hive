pub mod clip;
pub mod face;
pub mod llm;
pub mod model_manager;
pub mod ocr;

use std::sync::Mutex;

use clip::ClipModel;
use face::FaceModel;
use llm::ChatModel;
use ocr::OcrModel;

#[derive(Default)]
pub struct AiState {
    pub clip: Mutex<Option<ClipModel>>,
    pub ocr: Mutex<Option<OcrModel>>,
    pub face: Mutex<Option<FaceModel>>,
    pub llm: Mutex<Option<ChatModel>>,
}
