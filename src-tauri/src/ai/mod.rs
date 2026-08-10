pub mod aesthetic;
pub mod aesthetic_weights;
pub mod captioning;
pub mod clip;
pub mod cutout;
pub mod ddim;
pub mod face;
pub mod generate;
pub mod inpaint;
pub mod model_manager;
pub mod nsfw;
pub mod ocr;
pub mod segment;
pub mod session;
#[cfg(test)]
mod erase_check;
#[cfg(test)]
mod smoke;
pub mod upscale;

use std::sync::Mutex;

use captioning::CaptionModel;
use clip::ClipModel;
use cutout::CutoutModel;
use face::FaceModel;
use generate::GenerateModel;
use inpaint::InpaintModel;
use nsfw::NsfwModel;
use ocr::OcrModel;
use segment::{EncodedImage, SegmentModel};
use upscale::UpscaleModel;

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

    // Editor tools. Each stays loaded after first use, because an edit is
    // rarely a single operation and paying the load twice is felt.
    pub upscale: Mutex<Option<UpscaleModel>>,
    pub cutout: Mutex<Option<CutoutModel>>,
    pub segment: Mutex<Option<SegmentModel>>,
    pub inpaint: Mutex<Option<InpaintModel>>,
    /// Four sessions and 2 GB of weights, so it is loaded once and kept.
    pub generate: Mutex<Option<GenerateModel>>,
    /// The photo currently open for click-to-select, and its encoding.
    ///
    /// Only one is kept: an editor has one photo open, and each encoding is
    /// several megabytes. Clicking again on the same photo skips the expensive
    /// half entirely, which is the whole reason SAM is split in two.
    pub encoded: Mutex<Option<(String, EncodedImage)>>,

    /// A result the user has been shown but has not saved.
    ///
    /// This is what makes the AI tools behave like the sliders: nothing reaches
    /// the disk until Save is pressed. It also lets tools stack — erase, then
    /// enlarge what is left — because each one reads this before falling back to
    /// the file on disk.
    pub pending: Mutex<Option<PendingEdit>>,
}

/// An edit computed and previewed, still only in memory.
pub struct PendingEdit {
    /// The photo it belongs to; a different one discards it.
    pub media_id: String,
    /// What the tools have produced so far, at full resolution.
    pub image: image::DynamicImage,
    /// Names of the tools applied, oldest first — shown so the user can see
    /// what is waiting to be saved.
    pub steps: Vec<String>,
}
