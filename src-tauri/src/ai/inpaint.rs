//! Making something disappear: LaMa fills a masked region with plausible
//! background.
//!
//! What sets LaMa apart is Fourier convolutions. An ordinary convolutional
//! network sees only a small neighbourhood at each layer, so filling a large
//! hole means guessing from its rim — which is why such models smear. LaMa's
//! layers see the whole frame at once, so it can carry a brick course or a
//! horizon straight across the gap.
//!
//! The model takes a fixed 512×512. The photo is scaled down for inference and
//! only the masked area is pasted back at full size, so untouched pixels stay
//! exactly as sharp as they were.

use std::path::Path;

use image::{imageops::FilterType, GrayImage, RgbImage};
use ort::session::Session;
use ort::value::Tensor;

/// The size the export was frozen at.
const SIZE: u32 = 512;

/// How much of the surroundings to include beyond the marked area, as a share of
/// its longest side.
///
/// The model invents from what it can see. Handed the marked rectangle and
/// nothing else it has no wall, no horizon and no carpet to continue, and it
/// smears. Half again on each side gives it something to work from without
/// wasting the 512 pixels on parts of the photo nobody is changing.
const CONTEXT: f32 = 0.75;

/// Mask values at or above this count as "remove this".
const MASK_CUTOFF: u8 = 127;

/// The filled region is feathered back over this many pixels so the repair does
/// not end on a visible edge.
const FEATHER: i32 = 3;

/// How far to grow the marked area before filling, as a share of its longest
/// side.
///
/// Nobody draws a box exactly on an object's edge, and being a few pixels short
/// leaves a rim of it behind — which reads as "it did not erase properly"
/// rather than "the box was slightly too small". Growing the mark absorbs that,
/// and costs only a little more background being reinvented.
const GROW: f32 = 0.03;

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// The part of the photo worth sending to the model: what was marked, plus
/// enough of its surroundings to invent from.
///
/// This is what makes erasing on a large photo work at all. The model runs at
/// 512 pixels whatever it is given, so handing it the whole frame spends that
/// budget on the parts nobody is changing: a thumbnail marked in a 1300-pixel
/// screenshot arrives as 55 pixels, and 55 pixels of anything comes back as a
/// smudge. Cropping first spends all 512 on the area that matters.
fn work_region(mask: &GrayImage) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = mask.dimensions();
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0u32, 0u32);
    for (x, y, pixel) in mask.enumerate_pixels() {
        if pixel[0] >= MASK_CUTOFF {
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
    let pad = (marked * CONTEXT).round() as u32;
    let left = left.saturating_sub(pad);
    let top = top.saturating_sub(pad);
    let right = (right + pad).min(width - 1);
    let bottom = (bottom + pad).min(height - 1);

    Some((left, top, right - left + 1, bottom - top + 1))
}

/// Widens the marked area by a few pixels in every direction.
///
/// A box-shaped mark is grown by a box-shaped amount, which is all that is
/// needed: this is about covering the last rim of an object, not about tracing
/// it. Applied to a traced outline it simply thickens it, which is the same
/// help for the same reason.
fn grown(mask: &GrayImage) -> GrayImage {
    let (width, height) = mask.dimensions();
    let radius = ((width.max(height) as f32 * GROW).round() as i32).clamp(1, 24);

    // Two one-dimensional passes rather than one square window: the same result
    // for a box, and linear in the radius instead of quadratic.
    let mut horizontal = GrayImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let lit = (-radius..=radius).any(|dx| {
                let nx = x as i32 + dx;
                nx >= 0 && nx < width as i32 && mask.get_pixel(nx as u32, y)[0] >= MASK_CUTOFF
            });
            horizontal.put_pixel(x, y, image::Luma([if lit { 255 } else { 0 }]));
        }
    }

    let mut out = GrayImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let lit = (-radius..=radius).any(|dy| {
                let ny = y as i32 + dy;
                ny >= 0 && ny < height as i32 && horizontal.get_pixel(x, ny as u32)[0] >= MASK_CUTOFF
            });
            out.put_pixel(x, y, image::Luma([if lit { 255 } else { 0 }]));
        }
    }
    out
}

pub struct InpaintModel {
    session: Session,
    image_input: String,
    mask_input: String,
}

impl InpaintModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        // Processor only, deliberately — DirectML cannot run this model's
        // Fourier-convolution blocks. See `session::open_on_cpu`.
        let session = crate::ai::session::open_on_cpu(&dir.join("inpaint.onnx"))?;

        // Two inputs, and their order is not guaranteed across exports. The mask
        // is the single-channel one; asking the model beats assuming.
        let names: Vec<String> = session
            .inputs()
            .iter()
            .map(|input| input.name().to_string())
            .collect();
        let mask_input = names
            .iter()
            .find(|name| name.to_lowercase().contains("mask"))
            .cloned()
            .unwrap_or_else(|| names.get(1).cloned().unwrap_or_default());
        let image_input = names
            .iter()
            .find(|name| **name != mask_input)
            .cloned()
            .unwrap_or_default();

        if image_input.is_empty() || mask_input.is_empty() {
            anyhow::bail!("the inpainting model does not expose an image and a mask input");
        }

        Ok(Self {
            session,
            image_input,
            mask_input,
        })
    }

    /// Removes whatever the mask covers, returning the photo at its own size.
    ///
    /// `mask` must match `source`'s dimensions: white where something should
    /// disappear, black everywhere else.
    pub fn erase(&mut self, source: &RgbImage, mask: &GrayImage) -> anyhow::Result<RgbImage> {
        if mask.dimensions() != source.dimensions() {
            anyhow::bail!(
                "mask is {:?} but the photo is {:?}",
                mask.dimensions(),
                source.dimensions()
            );
        }
        // Grown first, so everything downstream — the window, the fill and the
        // paste — works on the same slightly wider area.
        let mask = &grown(mask);
        let Some((rx, ry, rw, rh)) = work_region(mask) else {
            anyhow::bail!("nothing was marked for removal");
        };

        // Only this window ever reaches the model, and only it comes back.
        let region = image::imageops::crop_imm(source, rx, ry, rw, rh).to_image();
        let region_mask = image::imageops::crop_imm(mask, rx, ry, rw, rh).to_image();

        let small = image::imageops::resize(&region, SIZE, SIZE, FilterType::Triangle);
        let small_mask = image::imageops::resize(&region_mask, SIZE, SIZE, FilterType::Triangle);

        let side = SIZE as usize;
        let mut pixels = vec![0.0f32; 3 * side * side];
        for (x, y, pixel) in small.enumerate_pixels() {
            for channel in 0..3 {
                pixels[channel * side * side + y as usize * side + x as usize] =
                    pixel[channel] as f32 / 255.0;
            }
        }
        let mut mask_values = vec![0.0f32; side * side];
        for (x, y, pixel) in small_mask.enumerate_pixels() {
            // Binary, not a gradient: LaMa treats the mask as "known" versus
            // "invent this", and a half value means neither.
            mask_values[y as usize * side + x as usize] =
                if pixel[0] >= MASK_CUTOFF { 1.0 } else { 0.0 };
        }

        let image_tensor =
            Tensor::from_array(([1usize, 3, side, side], pixels)).map_err(ort_err)?;
        let mask_tensor =
            Tensor::from_array(([1usize, 1, side, side], mask_values)).map_err(ort_err)?;

        let outputs = self
            .session
            .run(vec![
                (self.image_input.clone(), image_tensor.into_dyn()),
                (self.mask_input.clone(), mask_tensor.into_dyn()),
            ])
            .map_err(ort_err)?;

        let data: Vec<f32> = outputs[0]
            .try_extract_array::<f32>()
            .map_err(ort_err)?
            .iter()
            .copied()
            .collect();

        // LaMa's export returns 0..255 rather than 0..1. Deciding from the data
        // rather than the documentation, because the two disagree across exports
        // and the failure is a black rectangle, not an error.
        let peak = data.iter().copied().fold(0.0f32, f32::max);
        let scale = if peak > 1.5 { 1.0 } else { 255.0 };

        let mut filled = RgbImage::new(SIZE, SIZE);
        for y in 0..side {
            for x in 0..side {
                let mut rgb = [0u8; 3];
                for channel in 0..3 {
                    let value = data[channel * side * side + y * side + x] * scale;
                    rgb[channel] = value.clamp(0.0, 255.0).round() as u8;
                }
                filled.put_pixel(x as u32, y as u32, image::Rgb(rgb));
            }
        }

        // Back to the window's own size, then dropped into a copy of the photo
        // so everything outside the window is untouched to the pixel.
        let repaired_region =
            image::imageops::resize(&filled, rw, rh, FilterType::Lanczos3);
        let mut repaired = source.clone();
        for (x, y, pixel) in repaired_region.enumerate_pixels() {
            repaired.put_pixel(rx + x, ry + y, *pixel);
        }

        Ok(paste_repair(source, &repaired, mask))
    }
}

/// Keeps the original everywhere the mask is black, and fades into the repair
/// across the mask's edge.
///
/// Pasting only the masked pixels would leave a hard seam: the repair comes back
/// from a 512-pixel pass and is softer than its surroundings.
fn paste_repair(source: &RgbImage, repaired: &RgbImage, mask: &GrayImage) -> RgbImage {
    let (width, height) = source.dimensions();
    let mut out = source.clone();

    for y in 0..height {
        for x in 0..width {
            let weight = feathered_weight(mask, x as i32, y as i32, width, height);
            if weight <= 0.0 {
                continue;
            }
            let original = source.get_pixel(x, y);
            let fill = repaired.get_pixel(x, y);
            let mut blended = [0u8; 3];
            for channel in 0..3 {
                blended[channel] = (fill[channel] as f32 * weight
                    + original[channel] as f32 * (1.0 - weight))
                    .round() as u8;
            }
            out.put_pixel(x, y, image::Rgb(blended));
        }
    }
    out
}

/// 1 inside the mask, tapering to 0 just outside it.
fn feathered_weight(mask: &GrayImage, x: i32, y: i32, width: u32, height: u32) -> f32 {
    if mask.get_pixel(x as u32, y as u32)[0] >= MASK_CUTOFF {
        return 1.0;
    }
    // Outside: how close is the nearest masked pixel?
    let mut nearest = FEATHER + 1;
    for dy in -FEATHER..=FEATHER {
        for dx in -FEATHER..=FEATHER {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                continue;
            }
            if mask.get_pixel(nx as u32, ny as u32)[0] >= MASK_CUTOFF {
                nearest = nearest.min(dx.abs().max(dy.abs()));
            }
        }
    }
    if nearest > FEATHER {
        0.0
    } else {
        1.0 - nearest as f32 / (FEATHER as f32 + 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_with_square() -> GrayImage {
        let mut mask = GrayImage::new(20, 20);
        for y in 8..12 {
            for x in 8..12 {
                mask.put_pixel(x, y, image::Luma([255]));
            }
        }
        mask
    }

    #[test]
    fn the_work_region_hugs_the_mask_rather_than_the_photo() {
        // The whole point: a small mark in a large photo must not spend the
        // model's 512 pixels on the rest of the frame.
        let mut mask = GrayImage::new(1300, 900);
        for y in 400..460 {
            for x in 600..660 {
                mask.put_pixel(x, y, image::Luma([255]));
            }
        }

        let (x, y, w, h) = work_region(&mask).unwrap();
        assert!(w < 300 && h < 300, "region {w}×{h} is far too wide");
        // And it must still contain the mark, with room around it.
        assert!(x < 600 && y < 400);
        assert!(x + w > 660 && y + h > 460);
    }

    #[test]
    fn the_work_region_stays_inside_the_photo() {
        // A mark in the corner cannot pad past the edge.
        let mut mask = GrayImage::new(100, 80);
        mask.put_pixel(1, 1, image::Luma([255]));

        let (x, y, w, h) = work_region(&mask).unwrap();
        assert!(x + w <= 100 && y + h <= 80, "region {x},{y} {w}×{h} escapes");
    }

    #[test]
    fn an_empty_mask_has_no_region() {
        assert!(work_region(&GrayImage::new(40, 40)).is_none());
    }

    #[test]
    fn pixels_far_from_the_mask_are_left_exactly_alone() {
        let source = RgbImage::from_pixel(20, 20, image::Rgb([10, 20, 30]));
        let repaired = RgbImage::from_pixel(20, 20, image::Rgb([200, 200, 200]));

        let out = paste_repair(&source, &repaired, &mask_with_square());
        assert_eq!(out.get_pixel(0, 0), &image::Rgb([10, 20, 30]));
        assert_eq!(out.get_pixel(19, 19), &image::Rgb([10, 20, 30]));
    }

    #[test]
    fn pixels_inside_the_mask_come_entirely_from_the_repair() {
        let source = RgbImage::from_pixel(20, 20, image::Rgb([10, 20, 30]));
        let repaired = RgbImage::from_pixel(20, 20, image::Rgb([200, 200, 200]));

        let out = paste_repair(&source, &repaired, &mask_with_square());
        assert_eq!(out.get_pixel(9, 9), &image::Rgb([200, 200, 200]));
    }

    #[test]
    fn the_edge_fades_rather_than_stepping() {
        let mask = mask_with_square();
        // Just outside the square: partway between the two, not either extreme.
        let weight = feathered_weight(&mask, 7, 9, 20, 20);
        assert!(weight > 0.0 && weight < 1.0, "got {weight}");
        // Well outside: untouched.
        assert_eq!(feathered_weight(&mask, 2, 2, 20, 20), 0.0);
    }

    #[test]
    fn the_feather_reaches_exactly_as_far_as_it_claims() {
        let mask = mask_with_square();
        assert!(feathered_weight(&mask, 8 - FEATHER, 9, 20, 20) > 0.0);
        assert_eq!(feathered_weight(&mask, 8 - FEATHER - 1, 9, 20, 20), 0.0);
    }
}
