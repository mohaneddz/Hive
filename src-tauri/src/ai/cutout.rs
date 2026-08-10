//! Separating a subject from its background.
//!
//! Two models, because they are good at different things. MODNet was trained on
//! people and returns a soft matte that keeps hair as hair; ISNet is a general
//! salient-object model that handles products, animals and everything else, and
//! turns hair into a blob. Which one runs is decided by whether Hive already
//! found a face in the photo.
//!
//! Both answer with an **alpha matte**: one value per pixel, 0 for background,
//! 255 for subject, and everything in between at the edges. That in-between is
//! the whole point — a hard 0-or-1 cutout is what makes a composite look pasted.
//!
//! The obvious choice here was RMBG-1.4, which is what most tutorials reach for.
//! Its licence forbids commercial use. See `.idea/AI-Editor.md`.

use std::path::Path;

use image::{imageops::FilterType, GrayImage, RgbImage, RgbaImage};
use ort::session::Session;
use ort::value::Tensor;

/// MODNet is fully convolutional but wants a size divisible by 32.
const PORTRAIT_SIZE: u32 = 512;
/// ISNet was trained at this size and degrades noticeably away from it.
const GENERAL_SIZE: u32 = 1024;

/// 24 megapixels — more than any camera most people own, and the point past
/// which building the result costs more than computing it.
const MAX_PIXELS: u64 = 24_000_000;

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// Which model to run. `Portrait` is chosen when the photo has a face in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Portrait,
    General,
}

impl Subject {
    /// Hive already counts faces during indexing, so the choice costs nothing.
    pub fn for_face_count(faces: i64) -> Self {
        if faces > 0 {
            Subject::Portrait
        } else {
            Subject::General
        }
    }

    fn file(self) -> &'static str {
        match self {
            Subject::Portrait => "portrait.onnx",
            Subject::General => "general.onnx",
        }
    }

    fn size(self) -> u32 {
        match self {
            Subject::Portrait => PORTRAIT_SIZE,
            Subject::General => GENERAL_SIZE,
        }
    }

    /// MODNet takes pixels centred on zero; ISNet takes them in 0..1.
    fn normalise(self, value: f32) -> f32 {
        match self {
            Subject::Portrait => (value - 0.5) / 0.5,
            Subject::General => value,
        }
    }
}

pub struct CutoutModel {
    session: Session,
    subject: Subject,
    input_name: String,
}

impl CutoutModel {
    pub fn load(dir: &Path, subject: Subject) -> anyhow::Result<Self> {
        let session = crate::ai::session::open(&dir.join(subject.file()))?;
        let input_name = session
            .inputs()
            .first()
            .map(|input| input.name().to_string())
            .ok_or_else(|| anyhow::anyhow!("the cutout model declares no input"))?;
        Ok(Self {
            session,
            subject,
            input_name,
        })
    }

    pub fn subject(&self) -> Subject {
        self.subject
    }

    /// Refuses a photo too large to cut out without trouble.
    ///
    /// The matte itself is always computed at 1024, so size does not change how
    /// good the answer is — but the result is built at full resolution, and a
    /// 25-megapixel cutout means a 100 MB RGBA buffer and a PNG that takes
    /// seconds to encode. Enlarging a photo fourfold and then cutting it out
    /// reaches that in two clicks.
    pub fn check_size(width: u32, height: u32) -> anyhow::Result<()> {
        if u64::from(width) * u64::from(height) > MAX_PIXELS {
            anyhow::bail!(
                "{width}×{height} is too large to cut out. Cut the background out first, \
                 then enlarge the result — doing it the other way round means working on \
                 sixteen times more pixels for the same matte."
            );
        }
        Ok(())
    }

    /// Produces an alpha matte at the photo's own size.
    pub fn matte(&mut self, source: &RgbImage) -> anyhow::Result<GrayImage> {
        let side = self.subject.size();
        let scaled = image::imageops::resize(source, side, side, FilterType::Triangle);

        let (w, h) = (side as usize, side as usize);
        let mut pixels = vec![0.0f32; 3 * w * h];
        for (x, y, pixel) in scaled.enumerate_pixels() {
            for channel in 0..3 {
                pixels[channel * w * h + y as usize * w + x as usize] =
                    self.subject.normalise(pixel[channel] as f32 / 255.0);
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], pixels)).map_err(ort_err)?;
        let outputs = self
            .session
            .run(vec![(self.input_name.clone(), tensor.into_dyn())])
            .map_err(ort_err)?;

        // ISNet returns a stack of side outputs, the first being the finest.
        let shape: Vec<usize> = outputs[0].shape().iter().map(|&d| d as usize).collect();
        let (mask_h, mask_w) = match shape.as_slice() {
            [.., h, w] => (*h, *w),
            other => anyhow::bail!("the cutout model returned an unexpected shape {other:?}"),
        };
        let data: Vec<f32> = outputs[0]
            .try_extract_array::<f32>()
            .map_err(ort_err)?
            .iter()
            .copied()
            .collect();

        // ISNet's logits are unbounded, so the matte is rescaled by its own
        // range. MODNet already answers in 0..1 and this leaves it alone.
        let plane = &data[..mask_h * mask_w];
        let low = plane.iter().copied().fold(f32::INFINITY, f32::min);
        let high = plane.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let span = (high - low).max(1e-6);

        let mut matte = GrayImage::new(mask_w as u32, mask_h as u32);
        for y in 0..mask_h {
            for x in 0..mask_w {
                let value = ((plane[y * mask_w + x] - low) / span).clamp(0.0, 1.0);
                matte.put_pixel(x as u32, y as u32, image::Luma([(value * 255.0) as u8]));
            }
        }

        Ok(image::imageops::resize(
            &matte,
            source.width(),
            source.height(),
            FilterType::Triangle,
        ))
    }
}

/// Applies a matte as the alpha channel, giving a cut-out PNG.
pub fn apply_matte(source: &RgbImage, matte: &GrayImage) -> RgbaImage {
    let mut out = RgbaImage::new(source.width(), source.height());
    for (x, y, pixel) in source.enumerate_pixels() {
        let alpha = matte.get_pixel(x, y)[0];
        out.put_pixel(x, y, image::Rgba([pixel[0], pixel[1], pixel[2], alpha]));
    }
    out
}

/// Composites the subject over a replacement background, which is covered to fill
/// the frame rather than stretched — a squashed sky reads as fake instantly.
pub fn composite_over(source: &RgbImage, matte: &GrayImage, background: &RgbImage) -> RgbImage {
    let (width, height) = source.dimensions();
    let scale = (width as f32 / background.width() as f32)
        .max(height as f32 / background.height() as f32);
    let covered = image::imageops::resize(
        background,
        (background.width() as f32 * scale).ceil() as u32,
        (background.height() as f32 * scale).ceil() as u32,
        FilterType::Lanczos3,
    );
    let (offset_x, offset_y) = (
        (covered.width().saturating_sub(width)) / 2,
        (covered.height().saturating_sub(height)) / 2,
    );

    let mut out = RgbImage::new(width, height);
    for (x, y, pixel) in source.enumerate_pixels() {
        let alpha = matte.get_pixel(x, y)[0] as f32 / 255.0;
        let behind = covered.get_pixel(
            (x + offset_x).min(covered.width() - 1),
            (y + offset_y).min(covered.height() - 1),
        );
        let mut blended = [0u8; 3];
        for channel in 0..3 {
            blended[channel] = (pixel[channel] as f32 * alpha
                + behind[channel] as f32 * (1.0 - alpha))
                .round() as u8;
        }
        out.put_pixel(x, y, image::Rgb(blended));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_in_the_frame_picks_the_portrait_model() {
        assert_eq!(Subject::for_face_count(1), Subject::Portrait);
        assert_eq!(Subject::for_face_count(0), Subject::General);
    }

    #[test]
    fn the_two_models_want_pixels_scaled_differently() {
        // Feeding MODNet 0..1 instead of -1..1 does not fail, it just returns a
        // matte for a photo it never really saw.
        assert_eq!(Subject::Portrait.normalise(0.5), 0.0);
        assert_eq!(Subject::General.normalise(0.5), 0.5);
    }

    #[test]
    fn an_enlarged_photo_is_refused_before_it_becomes_a_problem() {
        // Enlarging 1881×853 fourfold and then cutting out is two clicks away,
        // and lands on 25 megapixels.
        let error = CutoutModel::check_size(7524, 3412).unwrap_err().to_string();
        assert!(error.contains("7524×3412"), "{error}");
        assert!(error.contains("enlarge the result"), "{error}");

        // A normal 12-megapixel photo goes straight through.
        assert!(CutoutModel::check_size(4000, 3000).is_ok());
    }

    #[test]
    fn a_matte_becomes_the_alpha_channel() {
        let mut source = RgbImage::new(2, 1);
        source.put_pixel(0, 0, image::Rgb([10, 20, 30]));
        source.put_pixel(1, 0, image::Rgb([40, 50, 60]));
        let mut matte = GrayImage::new(2, 1);
        matte.put_pixel(0, 0, image::Luma([255]));
        matte.put_pixel(1, 0, image::Luma([0]));

        let cut = apply_matte(&source, &matte);
        assert_eq!(cut.get_pixel(0, 0), &image::Rgba([10, 20, 30, 255]));
        assert_eq!(cut.get_pixel(1, 0), &image::Rgba([40, 50, 60, 0]));
    }

    #[test]
    fn a_half_transparent_edge_blends_rather_than_cuts() {
        let source = RgbImage::from_pixel(1, 1, image::Rgb([200, 200, 200]));
        let matte = GrayImage::from_pixel(1, 1, image::Luma([128]));
        let background = RgbImage::from_pixel(1, 1, image::Rgb([0, 0, 0]));

        // Half of 200 over half of 0, give or take the rounding of 128/255.
        let blended = composite_over(&source, &matte, &background);
        let value = blended.get_pixel(0, 0)[0];
        assert!((99..=101).contains(&value), "got {value}");
    }

    #[test]
    fn a_background_of_the_wrong_shape_is_covered_not_squashed() {
        let source = RgbImage::from_pixel(100, 100, image::Rgb([0, 0, 0]));
        let matte = GrayImage::from_pixel(100, 100, image::Luma([0]));
        // A wide background behind a square frame: it must be cropped, and the
        // result must still be exactly the source's size.
        let background = RgbImage::from_pixel(400, 100, image::Rgb([7, 7, 7]));

        let out = composite_over(&source, &matte, &background);
        assert_eq!(out.dimensions(), (100, 100));
        assert_eq!(out.get_pixel(50, 50), &image::Rgb([7, 7, 7]));
    }
}
