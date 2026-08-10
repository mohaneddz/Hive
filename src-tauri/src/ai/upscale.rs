//! Enlarging a photo with Real-ESRGAN general x4v3.
//!
//! The model quadruples each side, inventing detail rather than interpolating
//! it. It is the compact variant its authors trained on real photographs — the
//! ones carrying JPEG artefacts and sensor noise — which is why 4.9 MB is
//! enough.
//!
//! The photo is walked in tiles. A convolutional network does not need to see
//! the whole frame at once, and processing 4000×3000 in one tensor would ask for
//! memory no laptop GPU has. Tiles carry a margin of context on every side,
//! which is then cropped away: without it the seams show, because a pixel at the
//! edge of a tile has no neighbours to reason about.

use std::path::Path;

use image::{RgbImage, RgbaImage};
use ort::session::Session;
use ort::value::Tensor;

/// The side of the region actually kept from each pass.
const TILE: u32 = 256;
/// Context carried on every side and cropped off afterwards. 16 pixels is what
/// the receptive field needs to avoid a visible grid.
const OVERLAP: u32 = 16;
/// The model's fixed factor. Not a setting — it is baked into the weights.
pub const SCALE: u32 = 4;

/// Above this the result stops being useful and starts being a memory problem:
/// 64 megapixels is already an 8000×8000 image. A 12 MP photo enlarged fourfold
/// would be 192 MP and over half a gigabyte of pixels.
const MAX_OUTPUT_PIXELS: u64 = 64_000_000;

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

/// One pass: the region to read, and which part of the enlarged result to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tile {
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
    /// Offset of the kept area inside this tile's enlarged output.
    keep_x: u32,
    keep_y: u32,
    keep_w: u32,
    keep_h: u32,
    /// Where the kept area lands in the finished image.
    dst_x: u32,
    dst_y: u32,
}

/// Lays out the passes needed to cover `width` × `height`.
///
/// Every output pixel is written by exactly one tile: the margins overlap on the
/// way in, never on the way out.
fn plan_tiles(width: u32, height: u32, tile: u32, overlap: u32, scale: u32) -> Vec<Tile> {
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < height {
        let core_h = tile.min(height - y);
        let src_y = y.saturating_sub(overlap);
        let src_bottom = (y + core_h + overlap).min(height);

        let mut x = 0;
        while x < width {
            let core_w = tile.min(width - x);
            let src_x = x.saturating_sub(overlap);
            let src_right = (x + core_w + overlap).min(width);

            tiles.push(Tile {
                src_x,
                src_y,
                src_w: src_right - src_x,
                src_h: src_bottom - src_y,
                keep_x: (x - src_x) * scale,
                keep_y: (y - src_y) * scale,
                keep_w: core_w * scale,
                keep_h: core_h * scale,
                dst_x: x * scale,
                dst_y: y * scale,
            });
            x += tile;
        }
        y += tile;
    }
    tiles
}

pub struct UpscaleModel {
    session: Session,
    /// Read off the model rather than assumed — exports disagree on the name.
    input_name: String,
}

impl UpscaleModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let session = crate::ai::session::open(&dir.join("upscale.onnx"))?;
        let input_name = session
            .inputs()
            .first()
            .map(|input| input.name().to_string())
            .ok_or_else(|| anyhow::anyhow!("the upscaler declares no input"))?;
        Ok(Self {
            session,
            input_name,
        })
    }

    /// The size `enlarge` would produce, or an error if that is beyond reason.
    pub fn output_size(width: u32, height: u32) -> anyhow::Result<(u32, u32)> {
        let (out_w, out_h) = (width * SCALE, height * SCALE);
        if u64::from(out_w) * u64::from(out_h) > MAX_OUTPUT_PIXELS {
            anyhow::bail!(
                "{width}×{height} would become {out_w}×{out_h}, past what enlarging can \
                 usefully produce. Crop first, or start from a smaller photo."
            );
        }
        Ok((out_w, out_h))
    }

    /// Enlarges the image fourfold, reporting (tiles done, tiles total) as it goes.
    ///
    /// `should_stop` is polled between tiles so a long run can be abandoned
    /// without waiting for the whole photo.
    pub fn enlarge(
        &mut self,
        source: &RgbImage,
        mut on_progress: impl FnMut(usize, usize),
        should_stop: impl Fn() -> bool,
    ) -> anyhow::Result<RgbImage> {
        let (width, height) = source.dimensions();
        let (out_w, out_h) = Self::output_size(width, height)?;

        let tiles = plan_tiles(width, height, TILE, OVERLAP, SCALE);
        let total = tiles.len();
        let mut out = RgbImage::new(out_w, out_h);

        for (index, tile) in tiles.iter().enumerate() {
            if should_stop() {
                anyhow::bail!("cancelled");
            }

            let (w, h) = (tile.src_w as usize, tile.src_h as usize);
            let mut pixels = vec![0.0f32; 3 * w * h];
            for row in 0..h {
                for column in 0..w {
                    let pixel = source.get_pixel(tile.src_x + column as u32, tile.src_y + row as u32);
                    for channel in 0..3 {
                        // The model expects channel-first planes in 0..1.
                        pixels[channel * w * h + row * w + column] =
                            pixel[channel] as f32 / 255.0;
                    }
                }
            }

            let tensor = Tensor::from_array(([1usize, 3, h, w], pixels)).map_err(ort_err)?;
            let outputs = self
                .session
                .run(vec![(self.input_name.clone(), tensor.into_dyn())])
                .map_err(ort_err)?;

            let shape: Vec<usize> = outputs[0].shape().iter().map(|&d| d as usize).collect();
            let (scaled_h, scaled_w) = match shape.as_slice() {
                [_, _, h, w] => (*h, *w),
                other => anyhow::bail!("the upscaler returned an unexpected shape {other:?}"),
            };
            let data: Vec<f32> = outputs[0]
                .try_extract_array::<f32>()
                .map_err(ort_err)?
                .iter()
                .copied()
                .collect();

            for row in 0..tile.keep_h as usize {
                for column in 0..tile.keep_w as usize {
                    let (sy, sx) = (tile.keep_y as usize + row, tile.keep_x as usize + column);
                    if sy >= scaled_h || sx >= scaled_w {
                        continue;
                    }
                    let mut rgb = [0u8; 3];
                    for channel in 0..3 {
                        let value = data[channel * scaled_h * scaled_w + sy * scaled_w + sx];
                        rgb[channel] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                    out.put_pixel(
                        tile.dst_x + column as u32,
                        tile.dst_y + row as u32,
                        image::Rgb(rgb),
                    );
                }
            }

            on_progress(index + 1, total);
        }

        Ok(out)
    }
}

/// Drops transparency onto white. The upscaler takes three channels, and a PNG
/// with an alpha channel would otherwise be rejected at the tensor.
pub fn flatten_alpha(image: &RgbaImage) -> RgbImage {
    let mut out = RgbImage::new(image.width(), image.height());
    for (x, y, pixel) in image.enumerate_pixels() {
        let alpha = pixel[3] as f32 / 255.0;
        let blend = |channel: usize| {
            (pixel[channel] as f32 * alpha + 255.0 * (1.0 - alpha)).round() as u8
        };
        out.put_pixel(x, y, image::Rgb([blend(0), blend(1), blend(2)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_tile_covers_an_image_smaller_than_the_tile() {
        let tiles = plan_tiles(100, 80, TILE, OVERLAP, SCALE);
        assert_eq!(tiles.len(), 1);
        let only = &tiles[0];
        assert_eq!((only.src_w, only.src_h), (100, 80));
        assert_eq!((only.keep_w, only.keep_h), (400, 320));
        assert_eq!((only.dst_x, only.dst_y), (0, 0));
    }

    #[test]
    fn every_output_pixel_is_written_exactly_once() {
        // The property that matters: overlap happens on the way in, never out.
        // Written twice means seams, written zero times means holes.
        let (width, height) = (600u32, 400u32);
        let tiles = plan_tiles(width, height, TILE, OVERLAP, SCALE);

        let mut written = vec![0u8; (width * SCALE) as usize * (height * SCALE) as usize];
        for tile in &tiles {
            for row in 0..tile.keep_h {
                for column in 0..tile.keep_w {
                    let index = (tile.dst_y + row) as usize * (width * SCALE) as usize
                        + (tile.dst_x + column) as usize;
                    written[index] += 1;
                }
            }
        }

        assert!(written.iter().all(|count| *count == 1));
    }

    #[test]
    fn tiles_never_read_past_the_edge() {
        let (width, height) = (513u32, 257u32);
        for tile in plan_tiles(width, height, TILE, OVERLAP, SCALE) {
            assert!(tile.src_x + tile.src_w <= width, "{tile:?}");
            assert!(tile.src_y + tile.src_h <= height, "{tile:?}");
        }
    }

    #[test]
    fn inner_tiles_carry_context_on_every_side() {
        let tiles = plan_tiles(800, 800, TILE, OVERLAP, SCALE);
        // The second tile along has neighbours left and right, so it reads
        // TILE + two margins and keeps only the middle.
        let inner = tiles.iter().find(|t| t.dst_x == TILE * SCALE && t.dst_y == 0).unwrap();
        assert_eq!(inner.src_w, TILE + OVERLAP * 2);
        assert_eq!(inner.keep_x, OVERLAP * SCALE);
        assert_eq!(inner.keep_w, TILE * SCALE);
    }

    #[test]
    fn an_enormous_result_is_refused_with_a_reason() {
        // A 12 MP photo would become 192 MP.
        let error = UpscaleModel::output_size(4000, 3000).unwrap_err().to_string();
        assert!(error.contains("16000×12000"), "{error}");

        assert_eq!(UpscaleModel::output_size(1000, 800).unwrap(), (4000, 3200));
    }

    #[test]
    fn transparency_is_flattened_onto_white() {
        let mut image = RgbaImage::new(2, 1);
        image.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        image.put_pixel(1, 0, image::Rgba([255, 0, 0, 0]));

        let flat = flatten_alpha(&image);
        assert_eq!(flat.get_pixel(0, 0), &image::Rgb([255, 0, 0]));
        assert_eq!(flat.get_pixel(1, 0), &image::Rgb([255, 255, 255]));
    }
}
