//! NSFW (Not Safe For Work) content detection.
//!
//! Uses a lightweight MobileNetV2-based binary classifier that scores images
//! on a 0→1 scale: 0 = safe, 1 = nsfw. Images above a configurable threshold
//! are automatically hidden so they don't appear in the main gallery.
//!
//! Pipeline: resize 224×224 → ImageNet-normalise → inference → sigmoid → score

use std::path::Path;

use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;

/// These must match the model's own `preprocessor_config.json`. Feeding a model
/// pixels normalised the wrong way does not fail — it quietly returns nonsense,
/// which is far harder to notice than a crash.
///
/// AdamCodd/vit-base-nsfw-detector: 384×384, mean 0.5, std 0.5.
const IMAGE_SIZE: u32 = 384;
const PIXEL_MEAN: [f32; 3] = [0.5, 0.5, 0.5];
const PIXEL_STD: [f32; 3] = [0.5, 0.5, 0.5];

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

pub struct NsfwModel {
    session: Session,
}

impl NsfwModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let session = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("nsfw.onnx"))
            .map_err(ort_err)?;
        Ok(Self { session })
    }

    /// Scores the NSFW likelihood of an image. Returns a value between 0.0 and 1.0.
    /// 0 = safe, 1 = definitely NSFW.
    pub fn score(&mut self, path: &Path) -> anyhow::Result<f64> {
        let img = image::open(path)?
            .resize_exact(IMAGE_SIZE, IMAGE_SIZE, image::imageops::FilterType::Triangle)
            .to_rgb8();

        let (w, h) = (IMAGE_SIZE as usize, IMAGE_SIZE as usize);
        let mut pixels = vec![0.0f32; 3 * w * h];
        for (x, y, pixel) in img.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                let value = pixel[c] as f32 / 255.0;
                pixels[c * w * h + y * w + x] = (value - PIXEL_MEAN[c]) / PIXEL_STD[c];
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], pixels)).map_err(ort_err)?;
        let outputs = self.session.run(ort::inputs![tensor]).map_err(ort_err)?;

        let raw: Vec<f32> = outputs[0]
            .try_extract_array::<f32>()
            .map_err(ort_err)?
            .iter()
            .copied()
            .collect();

        // The model's config.json maps {0: "sfw", 1: "nsfw"}, so index 1 is the
        // one we want. Check that mapping again if the model is ever swapped.
        let nsfw_score = if raw.len() >= 2 {
            let max = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = raw.iter().map(|x| (x - max).exp()).collect();
            let sum: f32 = exps.iter().sum();
            (exps[1] / sum) as f64
        } else {
            // Single logit: sigmoid.
            1.0 / (1.0 + (-raw[0] as f64).exp())
        };

        Ok(nsfw_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the downloaded model, so it is skipped by default. Run it with
    /// `cargo test --lib nsfw -- --include-ignored --nocapture`.
    ///
    /// Two neutral pictures. What matters is not the exact figures but that the
    /// model runs at all and answers on the 0..1 scale the rest of the app reads
    /// — a wrong input size or a swapped label would show up here as a score
    /// pinned at one end, or as an outright failure.
    #[test]
    #[ignore]
    fn plain_images_run_through_the_model_and_score_low() {
        let Some(dir) = std::env::var_os("APPDATA")
            .map(|base| Path::new(&base).join("com.hive").join("models").join("nsfw"))
        else {
            return;
        };
        if !dir.join("nsfw.onnx").is_file() {
            return;
        }

        let mut model = NsfwModel::load(&dir).expect("model loads");
        println!("input: {}", model.session.inputs()[0].name());

        let samples: [(&str, image::RgbImage); 2] = [
            (
                "grey",
                image::RgbImage::from_pixel(400, 300, image::Rgb([128, 128, 128])),
            ),
            (
                "gradient",
                image::RgbImage::from_fn(400, 300, |x, y| {
                    image::Rgb([(x / 2) as u8, (y / 2) as u8, 90])
                }),
            ),
        ];

        for (name, image) in samples {
            let path = std::env::temp_dir().join(format!("hive_nsfw_{name}.png"));
            image.save(&path).unwrap();
            let score = model.score(&path).expect("scoring runs");
            println!("{name}: {score:.4}");

            assert!((0.0..=1.0).contains(&score), "{name} scored {score}");
            assert!(score < 0.5, "{name} scored {score}, labels may be swapped");
        }
    }
}
