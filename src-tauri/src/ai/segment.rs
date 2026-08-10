//! Click-to-select, with SlimSAM.
//!
//! SAM answers the question "what object is under this point?". SlimSAM is the
//! same model pruned to a fraction of its size — 13.8 MB against roughly 350 MB
//! for SAM ViT-B, which is what makes it shippable.
//!
//! It is split in two on purpose, and the split is what makes selecting feel
//! instant rather than like a request:
//!
//! ```text
//!   encoder (8.9 MB)   runs once per photo      →  image embedding, cached
//!   decoder (4.9 MB)   runs on every click      →  a mask, in milliseconds
//! ```
//!
//! Geometry is the fiddly part. SAM wants a 1024×1024 square, and a photo is
//! rarely square: it is scaled by its longest edge and the remainder is padded.
//! The click has to travel through exactly the same transform, or the mask comes
//! back for a different part of the picture — which looks like the model being
//! wrong rather than the arithmetic.

use std::collections::HashMap;
use std::path::Path;

use image::{imageops::FilterType, GrayImage, RgbImage};
use ort::session::Session;
use ort::value::{DynValue, Tensor};

/// The square SAM was trained on; the longest edge is scaled to this.
const SIDE: u32 = 1024;

/// From the model's own `preprocessor_config.json` — ImageNet statistics here,
/// unlike the 0.5/0.5 the captioning and NSFW models use.
const PIXEL_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const PIXEL_STD: [f32; 3] = [0.229, 0.224, 0.225];

/// Mask logits are centred on zero; above it means inside the object.
const LOGIT_CUTOFF: f32 = 0.0;

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// How a photo was fitted into SAM's square, kept so clicks can be mapped in and
/// masks mapped back out.
#[derive(Debug, Clone, Copy)]
struct Fit {
    scale: f32,
    used_w: u32,
    used_h: u32,
    source_w: u32,
    source_h: u32,
}

impl Fit {
    fn of(width: u32, height: u32) -> Self {
        let scale = SIDE as f32 / width.max(height) as f32;
        Fit {
            scale,
            used_w: (width as f32 * scale).round().min(SIDE as f32) as u32,
            used_h: (height as f32 * scale).round().min(SIDE as f32) as u32,
            source_w: width,
            source_h: height,
        }
    }

    /// A point in photo coordinates, in the square's coordinates.
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.scale, y * self.scale)
    }
}

/// One photo's encoding, reusable for every click on it.
pub struct EncodedImage {
    outputs: HashMap<String, (Vec<usize>, Vec<f32>)>,
    fit: Fit,
}

pub struct SegmentModel {
    encoder: Session,
    decoder: Session,
    encoder_input: String,
}

impl SegmentModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let encoder = crate::ai::session::open(&dir.join("encoder.onnx"))?;
        let decoder = crate::ai::session::open(&dir.join("decoder.onnx"))?;
        let encoder_input = encoder
            .inputs()
            .first()
            .map(|input| input.name().to_string())
            .ok_or_else(|| anyhow::anyhow!("the segmentation encoder declares no input"))?;
        Ok(Self {
            encoder,
            decoder,
            encoder_input,
        })
    }

    /// Runs the expensive half. Do this once, then click as often as you like.
    pub fn encode(&mut self, source: &RgbImage) -> anyhow::Result<EncodedImage> {
        let fit = Fit::of(source.width(), source.height());
        let scaled = image::imageops::resize(source, fit.used_w, fit.used_h, FilterType::Triangle);

        // Padded to the full square, bottom and right, as SAM expects.
        let side = SIDE as usize;
        let mut pixels = vec![0.0f32; 3 * side * side];
        for (x, y, pixel) in scaled.enumerate_pixels() {
            for channel in 0..3 {
                let value = pixel[channel] as f32 / 255.0;
                pixels[channel * side * side + y as usize * side + x as usize] =
                    (value - PIXEL_MEAN[channel]) / PIXEL_STD[channel];
            }
        }

        // Read before running: the outputs borrow the session for as long as they
        // live, and the names are needed alongside them.
        let names: Vec<String> = self
            .encoder
            .outputs()
            .iter()
            .map(|out| out.name().to_string())
            .collect();

        let tensor = Tensor::from_array(([1usize, 3, side, side], pixels)).map_err(ort_err)?;
        let produced = self
            .encoder
            .run(vec![(self.encoder_input.clone(), tensor.into_dyn())])
            .map_err(ort_err)?;

        // Kept by name: the decoder asks for what it needs, and exports differ on
        // whether positional embeddings come out of the encoder or not.
        let mut outputs = HashMap::new();
        for name in names {
            let value = &produced[name.as_str()];
            let shape = value.shape().iter().map(|&d| d as usize).collect();
            let data = value
                .try_extract_array::<f32>()
                .map_err(ort_err)?
                .iter()
                .copied()
                .collect();
            outputs.insert(name, (shape, data));
        }

        Ok(EncodedImage { outputs, fit })
    }

    /// The mask for the object under `(x, y)`, in photo coordinates.
    ///
    /// Extra points refine the selection: `true` means "this is also the object",
    /// `false` means "this is not" — which is how you carve a hole out of a
    /// selection that grabbed too much.
    pub fn mask_at(
        &mut self,
        encoded: &EncodedImage,
        points: &[(f32, f32, bool)],
    ) -> anyhow::Result<GrayImage> {
        if points.is_empty() {
            anyhow::bail!("no point was given to select from");
        }

        let mut coordinates = Vec::with_capacity(points.len() * 2);
        // Whole numbers, not floats. The decoder declares `input_labels` as
        // int64 and refuses a float outright — "Unexpected input data type" was
        // the entire error, with nothing to say which of the inputs it meant.
        let mut labels: Vec<i64> = Vec::with_capacity(points.len());
        for (x, y, positive) in points {
            let (px, py) = encoded.fit.point(*x, *y);
            coordinates.push(px);
            coordinates.push(py);
            labels.push(if *positive { 1 } else { 0 });
        }

        let count = points.len();
        let mut feeds: Vec<(String, DynValue)> = Vec::new();
        for input in self.decoder.inputs() {
            let name = input.name().to_string();
            let value = match name.as_str() {
                "input_points" => {
                    Tensor::from_array(([1usize, 1, count, 2], coordinates.clone()))
                        .map_err(ort_err)?
                        .into_dyn()
                }
                "input_labels" => Tensor::from_array(([1usize, 1, count], labels.clone()))
                    .map_err(ort_err)?
                    .into_dyn(),
                other => {
                    let (shape, data) = encoded.outputs.get(other).ok_or_else(|| {
                        anyhow::anyhow!(
                            "the decoder wants an input named `{other}` that the encoder does \
                             not produce — this export does not match the one Hive expects"
                        )
                    })?;
                    Tensor::from_array((shape.clone(), data.clone()))
                        .map_err(ort_err)?
                        .into_dyn()
                }
            };
            feeds.push((name, value));
        }

        let outputs = self.decoder.run(feeds).map_err(ort_err)?;

        // SAM proposes several masks and scores them; take the best-scoring one.
        // The alternative — always taking the first — picks a part of the object
        // as often as the object.
        let scores: Vec<f32> = outputs
            .iter()
            .find(|(name, _)| name.contains("iou"))
            .map(|(_, value)| {
                value
                    .try_extract_array::<f32>()
                    .map(|array| array.iter().copied().collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let masks = outputs
            .iter()
            .find(|(name, _)| name.contains("mask"))
            .map(|(_, value)| value)
            .ok_or_else(|| anyhow::anyhow!("the decoder returned no mask"))?;

        let shape: Vec<usize> = masks.shape().iter().map(|&d| d as usize).collect();
        let (candidates, mask_h, mask_w) = match shape.as_slice() {
            [.., c, h, w] => (*c, *h, *w),
            other => anyhow::bail!("unexpected mask shape {other:?}"),
        };
        let data: Vec<f32> = masks
            .try_extract_array::<f32>()
            .map_err(ort_err)?
            .iter()
            .copied()
            .collect();

        let best = scores
            .iter()
            .take(candidates)
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
            .unwrap_or(0);

        let plane = &data[best * mask_h * mask_w..(best + 1) * mask_h * mask_w];
        let mut square = GrayImage::new(mask_w as u32, mask_h as u32);
        for y in 0..mask_h {
            for x in 0..mask_w {
                let inside = plane[y * mask_w + x] > LOGIT_CUTOFF;
                square.put_pixel(x as u32, y as u32, image::Luma([if inside { 255 } else { 0 }]));
            }
        }

        Ok(unpad_to_source(&square, encoded.fit))
    }
}

/// Undoes the fit: crops away the padding, then scales back to the photo's size.
fn unpad_to_source(square: &GrayImage, fit: Fit) -> GrayImage {
    // The mask covers the padded square, so the useful part is the same
    // proportion of it that the photo was of the 1024 square.
    let used_w = ((fit.used_w as f32 / SIDE as f32) * square.width() as f32).round() as u32;
    let used_h = ((fit.used_h as f32 / SIDE as f32) * square.height() as f32).round() as u32;
    let cropped = image::imageops::crop_imm(square, 0, 0, used_w.max(1), used_h.max(1)).to_image();

    image::imageops::resize(&cropped, fit.source_w, fit.source_h, FilterType::Nearest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_landscape_photo_is_scaled_by_its_longest_edge() {
        let fit = Fit::of(2000, 1000);
        assert_eq!(fit.used_w, SIDE);
        assert_eq!(fit.used_h, SIDE / 2);
    }

    #[test]
    fn a_portrait_photo_is_scaled_by_its_longest_edge_too() {
        let fit = Fit::of(1000, 2000);
        assert_eq!(fit.used_w, SIDE / 2);
        assert_eq!(fit.used_h, SIDE);
    }

    #[test]
    fn a_click_lands_where_the_photo_was_put() {
        // The centre of a 2000×1000 photo must be the centre of the used area,
        // not the centre of the padded square — that is the bug this guards.
        let fit = Fit::of(2000, 1000);
        let (x, y) = fit.point(1000.0, 500.0);
        assert!((x - 512.0).abs() < 1.0, "got {x}");
        assert!((y - 256.0).abs() < 1.0, "got {y}");
    }

    #[test]
    fn a_small_photo_is_scaled_up_rather_than_left_in_a_corner() {
        let fit = Fit::of(200, 100);
        assert_eq!(fit.used_w, SIDE);
        assert!(fit.scale > 1.0);
    }

    #[test]
    fn the_mask_comes_back_at_the_photos_own_size() {
        let fit = Fit::of(2000, 1000);
        // A square mask, half of which is padding for this landscape photo.
        let mut square = GrayImage::new(256, 256);
        for y in 0..128 {
            for x in 0..256 {
                square.put_pixel(x, y, image::Luma([255]));
            }
        }

        let mask = unpad_to_source(&square, fit);
        assert_eq!(mask.dimensions(), (2000, 1000));
        // The whole used area was marked, so the whole photo comes back marked.
        assert_eq!(mask.get_pixel(1000, 500)[0], 255);
    }

    #[test]
    fn padding_is_cropped_off_instead_of_being_stretched_into_the_result() {
        let fit = Fit::of(2000, 1000);
        // Mark only the bottom half of the square — pure padding for this photo.
        let mut square = GrayImage::new(256, 256);
        for y in 128..256 {
            for x in 0..256 {
                square.put_pixel(x, y, image::Luma([255]));
            }
        }

        let mask = unpad_to_source(&square, fit);
        assert!(
            mask.pixels().all(|pixel| pixel[0] == 0),
            "padding leaked into the mask"
        );
    }
}
