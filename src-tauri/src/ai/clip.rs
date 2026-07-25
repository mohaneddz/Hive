use std::path::Path;

use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use tokenizers::Tokenizer;

const IMAGE_SIZE: u32 = 224;
const CLIP_MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const CLIP_STD: [f32; 3] = [0.268_629_54, 0.261_302_58, 0.275_777_11];
const MAX_TOKENS: usize = 77;
/// CLIP's `<|endoftext|>` token — also used as the pad token, matching the original
/// implementation's `argmax(input_ids)` trick for locating the true EOT position.
const EOT_TOKEN_ID: i64 = 49407;

/// ort's error type carries a non-Send/Sync builder-state marker, so it can't flow through
/// `anyhow`'s blanket `From` impl via `?` directly — stringify it immediately instead.
fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

pub struct ClipModel {
    vision: Session,
    text: Session,
    tokenizer: Tokenizer,
}

impl ClipModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let vision = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("vision_model.onnx"))
            .map_err(ort_err)?;
        let text = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("text_model.onnx"))
            .map_err(ort_err)?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| anyhow::anyhow!("failed to load CLIP tokenizer: {e}"))?;

        Ok(Self { vision, text, tokenizer })
    }

    /// Preprocesses an image (resize + center-crop to a square, CLIP mean/std normalize) and
    /// runs the CLIP vision tower, returning an L2-normalized 512-dim embedding.
    pub fn embed_image(&mut self, path: &Path) -> anyhow::Result<Vec<f32>> {
        let img = image::open(path)?;
        let resized = img.resize_to_fill(IMAGE_SIZE, IMAGE_SIZE, image::imageops::FilterType::Triangle);
        let rgb = resized.to_rgb8();

        let mut data = vec![0f32; 3 * IMAGE_SIZE as usize * IMAGE_SIZE as usize];
        let (w, h) = (IMAGE_SIZE as usize, IMAGE_SIZE as usize);
        for (x, y, pixel) in rgb.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                let value = pixel[c] as f32 / 255.0;
                let normalized = (value - CLIP_MEAN[c]) / CLIP_STD[c];
                data[c * w * h + y * w + x] = normalized;
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], data)).map_err(ort_err)?;
        let outputs = self
            .vision
            .run(ort::inputs!["pixel_values" => tensor])
            .map_err(ort_err)?;
        let embeds = outputs["image_embeds"].try_extract_array::<f32>().map_err(ort_err)?;
        let vec: Vec<f32> = embeds.iter().copied().collect();
        Ok(l2_normalize(vec))
    }

    /// Tokenizes `query` and runs the CLIP text tower, returning an L2-normalized 512-dim
    /// embedding comparable (via cosine similarity) against image embeddings.
    pub fn embed_text(&mut self, query: &str) -> anyhow::Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(query, true)
            .map_err(|e| anyhow::anyhow!("failed to tokenize query: {e}"))?;

        let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        ids.truncate(MAX_TOKENS);
        if ids.len() == MAX_TOKENS {
            if let Some(last) = ids.last_mut() {
                *last = EOT_TOKEN_ID;
            }
        }
        while ids.len() < MAX_TOKENS {
            ids.push(EOT_TOKEN_ID);
        }

        let seq_len = ids.len();
        let ids_tensor = Tensor::from_array(([1usize, seq_len], ids)).map_err(ort_err)?;

        let outputs = self
            .text
            .run(ort::inputs!["input_ids" => ids_tensor])
            .map_err(ort_err)?;
        let embeds = outputs["text_embeds"].try_extract_array::<f32>().map_err(ort_err)?;
        let vec: Vec<f32> = embeds.iter().copied().collect();
        Ok(l2_normalize(vec))
    }
}

fn l2_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    v
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn vector_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the real ONNX models against a local app-data cache. Ignored by default since
    /// it depends on the ~150MB CLIP download being present; run explicitly with
    /// `cargo test -- --ignored ai::clip::tests` after downloading via the Settings page.
    #[test]
    #[ignore]
    fn embeds_text_and_image_with_sane_geometry() {
        let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
            .join("com.hive")
            .join("models")
            .join("clip");
        let mut model = ClipModel::load(&dir).expect("failed to load CLIP model");

        let cat = model.embed_text("a photo of a cat").unwrap();
        let cat_again = model.embed_text("a photo of a cat").unwrap();
        let dog = model.embed_text("a photo of a dog").unwrap();

        assert_eq!(cat.len(), 512);
        let norm: f32 = cat.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "embedding should be L2-normalized, got norm {norm}");

        let self_sim = cosine_similarity(&cat, &cat_again);
        assert!(self_sim > 0.999, "identical text should embed identically, got {self_sim}");

        let cross_sim = cosine_similarity(&cat, &dog);
        assert!(cross_sim < self_sim, "distinct prompts should be less similar than identical ones");

        // Round-trip a synthetic image through the vision tower to prove the tensor
        // shapes/names line up end-to-end (content isn't meaningful, just shouldn't error).
        let tmp = std::env::temp_dir().join("hive_clip_test.png");
        let img = image::RgbImage::from_pixel(300, 200, image::Rgb([200, 120, 40]));
        img.save(&tmp).unwrap();
        let image_embed = model.embed_image(&tmp).unwrap();
        assert_eq!(image_embed.len(), 512);
        let img_norm: f32 = image_embed.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((img_norm - 1.0).abs() < 1e-3, "image embedding should be L2-normalized, got {img_norm}");
        let _ = std::fs::remove_file(&tmp);
    }
}
