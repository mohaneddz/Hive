pub mod clip;
pub mod model_manager;

use std::sync::Mutex;

use clip::ClipModel;

#[derive(Default)]
pub struct AiState {
    pub clip: Mutex<Option<ClipModel>>,
}
