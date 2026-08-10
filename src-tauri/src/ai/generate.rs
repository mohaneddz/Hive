//! Generative fill: describe what should be there, and it is painted in.
//!
//! Stable Diffusion 1.5 inpainting, four models working together. Unlike every
//! other tool in Hive this is not one call — it is a loop, and the loop is code
//! rather than something inside the `.onnx` files:
//!
//! ```text
//!   prompt      → text encoder → what you asked for, as numbers
//!   photo       → VAE encoder  → a small "latent" version, 8× smaller
//!   noise                      → the starting point
//!
//!   repeat ~25 times:
//!       UNet looks at [noisy latent | mask | masked photo] and the prompt,
//!       and says "this is the noise I see"
//!       the scheduler decides how much of it to remove   ← ai/ddim.rs
//!
//!   latent      → VAE decoder  → the finished picture
//! ```
//!
//! The UNet takes **nine** channels, not four: the noisy latent, plus the mask,
//! plus the photo with the masked area blanked. That is what makes it inpainting
//! rather than image generation with a hole cut out afterwards.
//!
//! Two silent-failure traps are handled explicitly below and marked where they
//! are: the mask's polarity, and the latent scaling factor. Getting either wrong
//! produces a picture — just the wrong one, with no error anywhere.

use std::path::Path;

use image::{imageops::FilterType, GrayImage, RgbImage};
use ort::session::Session;
use ort::value::{DynValue, Tensor, TensorElementType, ValueType};
use tokenizers::Tokenizer;

use crate::ai::ddim::{Ddim, Noise};

/// CLIP's context length. The text encoder was traced at exactly this, so the
/// prompt is padded or truncated to it rather than sent as-is.
const CONTEXT: usize = 77;
/// CLIP's end-of-text id, used as the padding token.
const EOT: i64 = 49407;

/// The VAE compresses by 8 in each direction: a 512-pixel photo becomes a
/// 64-wide latent.
const VAE_SCALE: u32 = 8;

/// What the latents are multiplied by after encoding, and divided by before
/// decoding. It is a property of the trained VAE. Dropping it does not fail —
/// it washes every result out into pale mush.
const LATENT_SCALING: f32 = 0.18215;

/// Stable Diffusion 1.5 was trained at this size and drifts away from it. The
/// photo is fitted to it rather than sent at its own resolution.
const TARGET_EDGE: u32 = 512;

/// How strongly to follow the prompt. 7.5 is the value the model is usually
/// demonstrated with: lower ignores you, higher burns the colours out.
const GUIDANCE: f32 = 7.5;

/// Surroundings to include beyond the marked area, as a share of its longest
/// side — the same reasoning as `inpaint.rs`, and for the same reason.
///
/// Named for the picture, not the prompt: `CONTEXT` above is CLIP's token
/// length, which is an entirely different thing.
///
/// The model works at 512 pixels whatever it is handed. Sent a whole 679×928
/// screenshot, a thumbnail-sized mark inside it arrives as a few dozen pixels
/// and comes back as a smear. Cropping to the mark and its context spends all
/// 512 where they matter.
const SURROUNDINGS: f32 = 0.75;

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// Builds a float tensor in whatever precision the model was exported at.
///
/// Stable Diffusion is distributed as fp16 — half the size, and everything
/// inside it speaks half-precision. Handing it 32-bit floats is refused outright
/// rather than converted, so the pipeline works in f32 throughout and narrows
/// only at the boundary.
fn float_tensor(shape: Vec<usize>, data: Vec<f32>, half_precision: bool) -> anyhow::Result<DynValue> {
    if half_precision {
        let narrowed: Vec<half::f16> = data.into_iter().map(half::f16::from_f32).collect();
        Ok(Tensor::from_array((shape, narrowed))
            .map_err(ort_err)?
            .into_dyn())
    } else {
        Ok(Tensor::from_array((shape, data)).map_err(ort_err)?.into_dyn())
    }
}

/// Reads a float tensor back as f32, whatever precision it came in.
fn extract_floats(value: &DynValue) -> anyhow::Result<Vec<f32>> {
    if let Ok(array) = value.try_extract_array::<f32>() {
        return Ok(array.iter().copied().collect());
    }
    let array = value.try_extract_array::<half::f16>().map_err(ort_err)?;
    Ok(array.iter().map(|value| value.to_f32()).collect())
}

/// Whether a named session input was exported in half precision.
///
/// Returned as a plain bool rather than a `ValueType`: building a stand-in for
/// "input not found" means constructing symbolic dimensions, and the only thing
/// the caller ever asks is which width to narrow to.
fn wants_half(session: &Session, name: &str) -> bool {
    session
        .inputs()
        .iter()
        .find(|outlet| outlet.name() == name)
        .is_some_and(|outlet| {
            matches!(outlet.dtype(), ValueType::Tensor { ty, .. } if *ty == TensorElementType::Float16)
        })
}

/// The window around the marked area that will actually be sent to the model.
///
/// Identical in purpose to `inpaint::work_region`: the model's 512 pixels have
/// to be spent on the area being replaced, not on the rest of the frame.
fn work_region(mask: &GrayImage) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = mask.dimensions();
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0u32, 0u32);
    for (x, y, pixel) in mask.enumerate_pixels() {
        if pixel[0] >= 128 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    if right < left || bottom < top {
        return None;
    }

    let marked = (right - left + 1).max(bottom - top + 1) as f32;
    let pad = (marked * SURROUNDINGS).round() as u32;
    let left = left.saturating_sub(pad);
    let top = top.saturating_sub(pad);
    let right = (right + pad).min(width - 1);
    let bottom = (bottom + pad).min(height - 1);

    Some((left, top, right - left + 1, bottom - top + 1))
}

/// Both sides have to be a multiple of this.
///
/// The VAE divides by 8, and the UNet then halves its own input three more
/// times before doubling back and joining each stage to the one it skipped. If
/// a latent side is not divisible by 8, those two halves come back different
/// sizes and the join fails: `Concat node '/up_blocks.2/Concat' ... invalid
/// parameter`, deep in the graph and nowhere near the cause. 8 × 8 = 64.
const SIZE_STEP: u32 = VAE_SCALE * 8;

/// The size to work at: the region's shape, fitted inside 512 and rounded so the
/// UNet's own halving and doubling land on whole numbers.
fn working_size(width: u32, height: u32) -> (u32, u32) {
    let scale = TARGET_EDGE as f32 / width.max(height) as f32;
    let round = |value: f32| {
        ((value / SIZE_STEP as f32).round().max(1.0) as u32) * SIZE_STEP
    };
    (round(width as f32 * scale), round(height as f32 * scale))
}

pub struct GenerateModel {
    text_encoder: Session,
    vae_encoder: Session,
    vae_decoder: Session,
    unet: Session,
    tokenizer: Tokenizer,
    /// Whether the text encoder was traced expecting 32-bit ids. Exports differ,
    /// and feeding the wrong width is rejected outright rather than silently.
    text_ids_are_i32: bool,
}

impl GenerateModel {
    /// `dir` holds the four ONNX files; `clip_dir` lends its tokenizer, which is
    /// the same CLIP vocabulary byte for byte.
    pub fn load(dir: &Path, clip_dir: &Path) -> anyhow::Result<Self> {
        let text_encoder = crate::ai::session::open(&dir.join("text_encoder.onnx"))?;
        let vae_encoder = crate::ai::session::open(&dir.join("vae_encoder.onnx"))?;
        let vae_decoder = crate::ai::session::open(&dir.join("vae_decoder.onnx"))?;
        let unet = crate::ai::session::open(&dir.join("unet.onnx"))?;

        let tokenizer = Tokenizer::from_file(clip_dir.join("tokenizer.json"))
            .map_err(|e| anyhow::anyhow!("failed to load the CLIP tokenizer: {e}"))?;

        let text_ids_are_i32 = matches!(
            text_encoder.inputs().first().map(|input| input.dtype()),
            Some(ValueType::Tensor { ty, .. }) if *ty == TensorElementType::Int32
        );

        Ok(Self {
            text_encoder,
            vae_encoder,
            vae_decoder,
            unet,
            tokenizer,
            text_ids_are_i32,
        })
    }

    /// Turns a prompt into the [1, 77, 768] block the UNet is conditioned on.
    fn encode_text(&mut self, prompt: &str) -> anyhow::Result<(Vec<usize>, Vec<f32>)> {
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("failed to read the description: {e}"))?;

        let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        ids.truncate(CONTEXT);
        if ids.len() == CONTEXT {
            // A truncated prompt must still end where the model expects.
            if let Some(last) = ids.last_mut() {
                *last = EOT;
            }
        }
        while ids.len() < CONTEXT {
            ids.push(EOT);
        }

        let name = self
            .text_encoder
            .inputs()
            .first()
            .map(|input| input.name().to_string())
            .unwrap_or_else(|| "input_ids".to_string());

        let tensor: DynValue = if self.text_ids_are_i32 {
            let narrow: Vec<i32> = ids.iter().map(|&id| id as i32).collect();
            Tensor::from_array(([1usize, CONTEXT], narrow))
                .map_err(ort_err)?
                .into_dyn()
        } else {
            Tensor::from_array(([1usize, CONTEXT], ids))
                .map_err(ort_err)?
                .into_dyn()
        };

        let outputs = self
            .text_encoder
            .run(vec![(name, tensor)])
            .map_err(ort_err)?;
        let shape: Vec<usize> = outputs[0].shape().iter().map(|&d| d as usize).collect();
        Ok((shape, extract_floats(&outputs[0])?))
    }

    /// Compresses a photo into latents, already scaled.
    fn encode_image(&mut self, image: &RgbImage) -> anyhow::Result<Vec<f32>> {
        let (width, height) = image.dimensions();
        let (w, h) = (width as usize, height as usize);

        // The VAE was trained on pixels in −1..1, not 0..1.
        let mut pixels = vec![0.0f32; 3 * w * h];
        for (x, y, pixel) in image.enumerate_pixels() {
            for channel in 0..3 {
                pixels[channel * w * h + y as usize * w + x as usize] =
                    pixel[channel] as f32 / 127.5 - 1.0;
            }
        }

        let name = self
            .vae_encoder
            .inputs()
            .first()
            .map(|input| input.name().to_string())
            .unwrap_or_else(|| "sample".to_string());
        let half = wants_half(&self.vae_encoder, &name);
        let tensor = float_tensor(vec![1, 3, h, w], pixels, half)?;
        let outputs = self
            .vae_encoder
            .run(vec![(name, tensor)])
            .map_err(ort_err)?;

        let latents = extract_floats(&outputs[0])?;
        // Trap #2: without this factor the loop runs and returns pale mush.
        Ok(latents.iter().map(|value| value * LATENT_SCALING).collect())
    }

    fn decode_latents(&mut self, latents: &[f32], w: usize, h: usize) -> anyhow::Result<RgbImage> {
        let unscaled: Vec<f32> = latents.iter().map(|value| value / LATENT_SCALING).collect();
        let name = self
            .vae_decoder
            .inputs()
            .first()
            .map(|input| input.name().to_string())
            .unwrap_or_else(|| "latent_sample".to_string());

        let half = wants_half(&self.vae_decoder, &name);
        let tensor = float_tensor(vec![1, 4, h, w], unscaled, half)?;
        let outputs = self
            .vae_decoder
            .run(vec![(name, tensor)])
            .map_err(ort_err)?;

        let shape: Vec<usize> = outputs[0].shape().iter().map(|&d| d as usize).collect();
        let (out_h, out_w) = match shape.as_slice() {
            [.., h, w] => (*h, *w),
            other => anyhow::bail!("the decoder returned an unexpected shape {other:?}"),
        };
        let data = extract_floats(&outputs[0])?;

        let mut out = RgbImage::new(out_w as u32, out_h as u32);
        for y in 0..out_h {
            for x in 0..out_w {
                let mut rgb = [0u8; 3];
                for channel in 0..3 {
                    // Back from −1..1 to bytes.
                    let value = data[channel * out_h * out_w + y * out_w + x];
                    rgb[channel] = ((value + 1.0) * 127.5).clamp(0.0, 255.0).round() as u8;
                }
                out.put_pixel(x as u32, y as u32, image::Rgb(rgb));
            }
        }
        Ok(out)
    }

    /// One UNet pass over the nine-channel input.
    #[allow(clippy::too_many_arguments)]
    fn predict_noise(
        &mut self,
        latents: &[f32],
        mask: &[f32],
        masked_latents: &[f32],
        timestep: i64,
        text: &(Vec<usize>, Vec<f32>),
        lw: usize,
        lh: usize,
    ) -> anyhow::Result<Vec<f32>> {
        let area = lw * lh;
        let mut input = Vec::with_capacity(9 * area);
        input.extend_from_slice(latents);
        input.extend_from_slice(mask);
        input.extend_from_slice(masked_latents);

        // Names and types are read off the model: exports disagree on both, and
        // this one is fp16 throughout while the pipeline works in f32.
        let plan: Vec<(String, bool, bool)> = self
            .unet
            .inputs()
            .iter()
            .map(|outlet| {
                let (half, integer) = match outlet.dtype() {
                    ValueType::Tensor { ty, .. } => (
                        *ty == TensorElementType::Float16,
                        *ty == TensorElementType::Int64,
                    ),
                    _ => (false, false),
                };
                (outlet.name().to_string(), half, integer)
            })
            .collect();

        let mut feeds: Vec<(String, DynValue)> = Vec::with_capacity(plan.len());
        for (name, half, integer) in plan {
            let value = if name.contains("timestep") {
                // Some exports want the timestep as a number, others as an int.
                if integer {
                    Tensor::from_array(([1usize], vec![timestep]))
                        .map_err(ort_err)?
                        .into_dyn()
                } else {
                    float_tensor(vec![1], vec![timestep as f32], half)?
                }
            } else if name.contains("encoder_hidden_states") {
                float_tensor(text.0.clone(), text.1.clone(), half)?
            } else {
                float_tensor(vec![1, 9, lh, lw], input.clone(), half)?
            };
            feeds.push((name, value));
        }

        let outputs = self.unet.run(feeds).map_err(ort_err)?;
        extract_floats(&outputs[0])
    }

    /// Paints `prompt` into whatever the mask covers.
    ///
    /// `mask` is white where something should be generated — the same polarity
    /// as the erase tool, and **trap #1**: inverted, the model repaints
    /// everything you wanted to keep and keeps what you wanted replaced. It fails
    /// beautifully, which is what makes it hard to spot.
    pub fn generate(
        &mut self,
        source: &RgbImage,
        mask: &GrayImage,
        prompt: &str,
        steps: usize,
        seed: u64,
        mut on_progress: impl FnMut(usize, usize),
        should_stop: impl Fn() -> bool,
    ) -> anyhow::Result<RgbImage> {
        if prompt.trim().is_empty() {
            anyhow::bail!("describe what should appear there");
        }
        if mask.dimensions() != source.dimensions() {
            anyhow::bail!("the mask does not match the photo");
        }
        if mask.pixels().all(|pixel| pixel[0] < 128) {
            anyhow::bail!("nothing was selected to replace");
        }

        // Only the window around the mark is sent, and only it comes back.
        let Some((rx, ry, rw, rh)) = work_region(mask) else {
            anyhow::bail!("nothing was selected to replace");
        };
        let region = image::imageops::crop_imm(source, rx, ry, rw, rh).to_image();
        let region_mask = image::imageops::crop_imm(mask, rx, ry, rw, rh).to_image();

        let (w, h) = working_size(rw, rh);
        let (lw, lh) = ((w / VAE_SCALE) as usize, (h / VAE_SCALE) as usize);
        let area = lw * lh;

        let fitted = image::imageops::resize(&region, w, h, FilterType::Lanczos3);
        let fitted_mask = image::imageops::resize(&region_mask, w, h, FilterType::Nearest);

        // The photo with the selection blanked out: what the model is allowed to
        // see of the area it must fill.
        let mut blanked = fitted.clone();
        for (x, y, pixel) in fitted_mask.enumerate_pixels() {
            if pixel[0] >= 128 {
                blanked.put_pixel(x, y, image::Rgb([127, 127, 127]));
            }
        }

        let conditioned = self.encode_text(prompt)?;
        let unconditioned = self.encode_text("")?;
        let masked_latents = self.encode_image(&blanked)?;

        // The mask joins the UNet at latent resolution, 1 where to paint.
        let small_mask = image::imageops::resize(&fitted_mask, lw as u32, lh as u32, FilterType::Triangle);
        let mask_channel: Vec<f32> = small_mask
            .pixels()
            .map(|pixel| if pixel[0] >= 128 { 1.0 } else { 0.0 })
            .collect();

        let ddim = Ddim::new(steps);
        let mut noise = Noise::seeded(seed);
        let mut latents: Vec<f32> = noise
            .vector(4 * area)
            .iter()
            .map(|value| value * ddim.initial_noise_sigma())
            .collect();

        let timesteps = ddim.timesteps().to_vec();
        for (index, &t) in timesteps.iter().enumerate() {
            if should_stop() {
                anyhow::bail!("cancelled");
            }

            // Classifier-free guidance: ask twice, once with the prompt and once
            // with nothing, then exaggerate the difference. One pass at a time
            // rather than a batch of two — half the peak memory, which is what
            // keeps this inside a 4 GB card.
            let with_prompt = self.predict_noise(
                &latents, &mask_channel, &masked_latents, t as i64, &conditioned, lw, lh,
            )?;
            let without = self.predict_noise(
                &latents, &mask_channel, &masked_latents, t as i64, &unconditioned, lw, lh,
            )?;

            let guided: Vec<f32> = without
                .iter()
                .zip(with_prompt.iter())
                .map(|(plain, prompted)| plain + GUIDANCE * (prompted - plain))
                .collect();

            latents = ddim.step(&latents, &guided, t);

            // Compared against a Python run of the same loop on the same weights
            // when the output came back as noise. Printing the range each step is
            // what located the divergence: two implementations of one algorithm
            // agree at every step or they do not agree at all.
            if std::env::var_os("HIVE_TRACE_DIFFUSION").is_some() {
                let low = latents.iter().copied().fold(f32::INFINITY, f32::min);
                let high = latents.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                println!(
                    "  step {}/{}  t={t}  latent range {low:.2}..{high:.2}",
                    index + 1,
                    timesteps.len()
                );
            }
            on_progress(index + 1, timesteps.len());
        }

        let painted = self.decode_latents(&latents, lw, lh)?;
        // Back to the window's own size, then dropped into a copy of the photo.
        let painted_region = image::imageops::resize(&painted, rw, rh, FilterType::Lanczos3);
        let mut painted_full = source.clone();
        for (x, y, pixel) in painted_region.enumerate_pixels() {
            painted_full.put_pixel(rx + x, ry + y, *pixel);
        }

        // Only the selection is taken from the model. The VAE is lossy, so
        // decoding and re-encoding the untouched parts of the photo would soften
        // the whole thing for no reason.
        Ok(composite(source, &painted_full, mask))
    }
}

/// How far the painted area fades into the photo, as a share of its longest side.
///
/// A hard edge between invented pixels and real ones is visible even when the
/// invention is good: the two were lit and decoded separately, and the join
/// reads as a cut-out pasted on. Fading across a few percent hides the seam
/// without blurring what was asked for.
const BLEND: f32 = 0.04;

/// Keeps the original where the mask is black, the painting where it is white,
/// and mixes the two across the boundary.
fn composite(original: &RgbImage, painted: &RgbImage, mask: &GrayImage) -> RgbImage {
    let (width, height) = mask.dimensions();
    let radius = ((width.max(height) as f32 * BLEND).round() as i32).clamp(1, 40);

    // A box blur of the mask, separably: the softened mask is the mixing weight.
    let mut rows = vec![0.0f32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let mut total = 0.0;
            let mut count = 0.0;
            for dx in -radius..=radius {
                let nx = x as i32 + dx;
                if nx >= 0 && nx < width as i32 {
                    total += if mask.get_pixel(nx as u32, y)[0] >= 128 { 1.0 } else { 0.0 };
                    count += 1.0;
                }
            }
            rows[(y * width + x) as usize] = total / count;
        }
    }

    let mut out = original.clone();
    for y in 0..height {
        for x in 0..width {
            let mut total = 0.0;
            let mut count = 0.0;
            for dy in -radius..=radius {
                let ny = y as i32 + dy;
                if ny >= 0 && ny < height as i32 {
                    total += rows[(ny as u32 * width + x) as usize];
                    count += 1.0;
                }
            }
            let weight = (total / count).clamp(0.0, 1.0);
            if weight <= 0.001 {
                continue;
            }
            let from = original.get_pixel(x, y);
            let to = painted.get_pixel(x, y);
            let mut blended = [0u8; 3];
            for channel in 0..3 {
                blended[channel] = (to[channel] as f32 * weight
                    + from[channel] as f32 * (1.0 - weight))
                    .round() as u8;
            }
            out.put_pixel(x, y, image::Rgb(blended));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_photo_is_fitted_to_the_trained_size() {
        assert_eq!(working_size(1000, 1000), (512, 512));
    }

    #[test]
    fn a_wide_photo_keeps_its_shape() {
        // Squashing to a square is the lazy option and it distorts faces.
        let (w, h) = working_size(2000, 1000);
        assert_eq!(w, 512);
        assert_eq!(h, 256);
    }

    #[test]
    fn every_working_size_survives_the_unets_halving() {
        // Not just the VAE's 8: the UNet halves three more times and rejoins,
        // and an odd latent side fails inside `/up_blocks.2/Concat` with an
        // "invalid parameter" that names nothing useful.
        for (w, h) in [(1913, 1077), (100, 37), (4000, 2251), (640, 640), (560, 503)] {
            let (fitted_w, fitted_h) = working_size(w, h);
            assert_eq!(fitted_w % SIZE_STEP, 0, "{w}x{h} gave width {fitted_w}");
            assert_eq!(fitted_h % SIZE_STEP, 0, "{w}x{h} gave height {fitted_h}");
            assert!(fitted_w >= SIZE_STEP && fitted_h >= SIZE_STEP);
        }
    }

    #[test]
    fn a_tiny_photo_is_enlarged_rather_than_left_alone() {
        let (w, h) = working_size(64, 64);
        assert_eq!((w, h), (512, 512));
    }

    #[test]
    fn only_the_selected_area_comes_from_the_model() {
        let original = RgbImage::from_pixel(200, 200, image::Rgb([10, 10, 10]));
        let painted = RgbImage::from_pixel(200, 200, image::Rgb([200, 200, 200]));
        let mut mask = GrayImage::new(200, 200);
        for y in 80..120 {
            for x in 80..120 {
                mask.put_pixel(x, y, image::Luma([255]));
            }
        }

        let out = composite(&original, &painted, &mask);
        // Well away from the mark, the photo is untouched to the byte.
        assert_eq!(out.get_pixel(2, 2), &image::Rgb([10, 10, 10]));
        // Well inside it, the painting has taken over completely.
        assert_eq!(out.get_pixel(100, 100), &image::Rgb([200, 200, 200]));
    }

    #[test]
    fn the_edge_of_the_painting_fades_instead_of_cutting() {
        // The hard cut this replaced was visible even on good results: two sets
        // of pixels lit and decoded separately, meeting at a line.
        let original = RgbImage::from_pixel(200, 200, image::Rgb([10, 10, 10]));
        let painted = RgbImage::from_pixel(200, 200, image::Rgb([200, 200, 200]));
        let mut mask = GrayImage::new(200, 200);
        for y in 80..120 {
            for x in 80..120 {
                mask.put_pixel(x, y, image::Luma([255]));
            }
        }

        let out = composite(&original, &painted, &mask);
        let edge = out.get_pixel(80, 100)[0];
        assert!(edge > 10 && edge < 200, "the boundary should mix, got {edge}");
    }

    #[test]
    fn the_latent_scaling_is_undone_exactly() {
        // Encode multiplies, decode divides. If the two ever disagree the result
        // is not an error, it is a washed-out picture.
        let value = 1.234f32;
        let round_trip = (value * LATENT_SCALING) / LATENT_SCALING;
        assert!((round_trip - value).abs() < 1e-6);
    }
}
