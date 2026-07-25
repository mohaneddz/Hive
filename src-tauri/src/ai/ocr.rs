use std::collections::VecDeque;
use std::path::Path;

use image::{DynamicImage, GenericImageView, Rgb};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;

/// PaddleOCR PP-OCRv5 detection + English recognition, run locally via ONNX Runtime.
///
/// Known simplification vs. the reference PaddleOCR pipeline: text regions are extracted as
/// axis-aligned bounding boxes (simple 4-connected flood fill + "unclip" padding) rather than
/// PaddleOCR's rotated minimum-area-rect + Vatti polygon offset. This works well for the
/// horizontal-ish text found in most photos (signs, labels, screenshots, book covers) but will
/// under-crop text that's rotated more than a few degrees.
pub struct OcrModel {
    det: Session,
    rec: Session,
    /// Index 0 is unused (CTC blank), 1..=N are `dict.txt` lines, N+1 is space —
    /// matches PaddleOCR's `['blank'] + dict + [' ']` class layout.
    dict: Vec<String>,
}

const DET_LIMIT_SIDE: u32 = 960;
const DET_THRESH: f32 = 0.3;
const MIN_BOX_AREA: u32 = 24;
const UNCLIP_RATIO: f32 = 0.55;
const REC_HEIGHT: u32 = 48;
const REC_MAX_WIDTH: u32 = 480;

const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

struct BBox {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl OcrModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let det = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("det.onnx"))
            .map_err(ort_err)?;
        let rec = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("rec.onnx"))
            .map_err(ort_err)?;

        let dict_text = std::fs::read_to_string(dir.join("dict.txt"))?;
        let mut dict: Vec<String> = dict_text.lines().map(|l| l.to_string()).collect();
        dict.push(" ".to_string());

        Ok(Self { det, rec, dict })
    }

    /// Detects text regions and recognizes each, returning the page's text as a single
    /// newline-joined string ordered top-to-bottom, left-to-right. Returns an empty string
    /// (not an error) when no text is found.
    pub fn extract_text(&mut self, path: &Path) -> anyhow::Result<String> {
        let img = image::open(path)?;
        let boxes = self.detect_boxes(&img)?;
        if boxes.is_empty() {
            return Ok(String::new());
        }

        let mut ordered = boxes;
        ordered.sort_by_key(|b| (b.y0 / 20, b.x0));

        let mut lines = Vec::with_capacity(ordered.len());
        for bbox in &ordered {
            let cropped = img.crop_imm(bbox.x0, bbox.y0, bbox.x1 - bbox.x0, bbox.y1 - bbox.y0);
            if let Ok(text) = self.recognize(&cropped) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
        }

        Ok(lines.join("\n"))
    }

    fn detect_boxes(&mut self, img: &DynamicImage) -> anyhow::Result<Vec<BBox>> {
        let (orig_w, orig_h) = img.dimensions();
        let scale = (DET_LIMIT_SIDE as f32 / orig_w.max(orig_h) as f32).min(1.0);
        let resize_w = (((orig_w as f32 * scale) / 32.0).round().max(1.0) as u32) * 32;
        let resize_h = (((orig_h as f32 * scale) / 32.0).round().max(1.0) as u32) * 32;

        let resized = img.resize_exact(resize_w, resize_h, image::imageops::FilterType::Triangle);
        let rgb = resized.to_rgb8();

        let mut data = vec![0f32; 3 * resize_h as usize * resize_w as usize];
        let (w, h) = (resize_w as usize, resize_h as usize);
        for (x, y, pixel) in rgb.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                let value = pixel[c] as f32 / 255.0;
                data[c * w * h + y * w + x] = (value - IMAGENET_MEAN[c]) / IMAGENET_STD[c];
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], data)).map_err(ort_err)?;
        let outputs = self.det.run(ort::inputs!["x" => tensor]).map_err(ort_err)?;
        let (shape, prob) = outputs["fetch_name_0"].try_extract_tensor::<f32>().map_err(ort_err)?;
        let map_h = shape[2] as usize;
        let map_w = shape[3] as usize;

        let mut visited = vec![false; map_h * map_w];
        let mut boxes = Vec::new();

        for start in 0..(map_h * map_w) {
            if visited[start] || prob[start] < DET_THRESH {
                continue;
            }
            // 4-connected flood fill over the thresholded probability map.
            let mut queue = VecDeque::new();
            queue.push_back(start);
            visited[start] = true;
            let (mut min_x, mut max_x, mut min_y, mut max_y) = (map_w, 0usize, map_h, 0usize);
            let mut count = 0u32;

            while let Some(idx) = queue.pop_front() {
                let (x, y) = (idx % map_w, idx / map_w);
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                count += 1;

                let neighbors = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];
                for (nx, ny) in neighbors {
                    if nx < map_w && ny < map_h {
                        let nidx = ny * map_w + nx;
                        if !visited[nidx] && prob[nidx] >= DET_THRESH {
                            visited[nidx] = true;
                            queue.push_back(nidx);
                        }
                    }
                }
            }

            if count < MIN_BOX_AREA {
                continue;
            }

            // Scale back to original image coordinates and pad ("unclip") for a small margin.
            let sx = orig_w as f32 / map_w as f32;
            let sy = orig_h as f32 / map_h as f32;
            let box_h = ((max_y - min_y + 1) as f32) * sy;
            let pad = (box_h * UNCLIP_RATIO).max(2.0);

            let x0 = ((min_x as f32) * sx - pad).max(0.0) as u32;
            let y0 = ((min_y as f32) * sy - pad).max(0.0) as u32;
            let x1 = (((max_x + 1) as f32) * sx + pad).min(orig_w as f32) as u32;
            let y1 = (((max_y + 1) as f32) * sy + pad).min(orig_h as f32) as u32;

            if x1 > x0 && y1 > y0 {
                boxes.push(BBox { x0, y0, x1, y1 });
            }
        }

        Ok(boxes)
    }

    fn recognize(&mut self, crop: &DynamicImage) -> anyhow::Result<String> {
        let (cw, ch) = crop.dimensions();
        if cw == 0 || ch == 0 {
            return Ok(String::new());
        }
        let target_w = ((cw as f32 * REC_HEIGHT as f32 / ch as f32).round() as u32)
            .clamp(REC_HEIGHT, REC_MAX_WIDTH);

        let resized = crop.resize_exact(target_w, REC_HEIGHT, image::imageops::FilterType::Triangle);
        let rgb = resized.to_rgb8();

        let (w, h) = (target_w as usize, REC_HEIGHT as usize);
        let mut data = vec![0f32; 3 * h * w];
        for (x, y, Rgb(pixel)) in rgb.enumerate_pixels().map(|(x, y, p)| (x, y, *p)) {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                let value = pixel[c] as f32 / 255.0;
                data[c * w * h + y * w + x] = (value - 0.5) / 0.5;
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], data)).map_err(ort_err)?;
        let outputs = self.rec.run(ort::inputs!["x" => tensor]).map_err(ort_err)?;
        let (shape, logits) = outputs["fetch_name_0"].try_extract_tensor::<f32>().map_err(ort_err)?;
        let seq_len = shape[1] as usize;
        let num_classes = shape[2] as usize;

        let mut result = String::new();
        let mut prev_class: Option<usize> = None;
        for t in 0..seq_len {
            let row = &logits[t * num_classes..(t + 1) * num_classes];
            let (best_idx, _) = row
                .iter()
                .enumerate()
                .fold((0usize, f32::MIN), |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc });

            if best_idx != 0 && Some(best_idx) != prev_class {
                if let Some(ch) = self.dict.get(best_idx - 1) {
                    result.push_str(ch);
                }
            }
            prev_class = if best_idx == 0 { None } else { Some(best_idx) };
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders real text onto a synthetic image with a system font and runs it through the
    /// full detect+recognize pipeline. Ignored by default since it depends on the OCR model
    /// download; run explicitly with `cargo test -- --ignored ai::ocr::tests`.
    #[test]
    #[ignore]
    fn extracts_rendered_text_from_a_synthetic_image() {
        use ab_glyph::{FontRef, PxScale};
        use imageproc::drawing::draw_text_mut;

        let dir = std::path::PathBuf::from(std::env::var("APPDATA").unwrap())
            .join("com.hive")
            .join("models")
            .join("ocr");
        let mut model = OcrModel::load(&dir).expect("failed to load OCR model");

        let font_bytes = std::fs::read("C:/Windows/Fonts/arial.ttf").expect("system font not found");
        let font = FontRef::try_from_slice(&font_bytes).unwrap();

        let mut img = image::RgbImage::from_pixel(500, 150, image::Rgb([255, 255, 255]));
        draw_text_mut(
            &mut img,
            image::Rgb([0, 0, 0]),
            30,
            50,
            PxScale::from(48.0),
            &font,
            "HELLO WORLD",
        );

        let tmp = std::env::temp_dir().join("hive_ocr_test.png");
        img.save(&tmp).unwrap();

        let text = model.extract_text(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);

        let normalized: String = text.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            normalized.contains("HELLO") || normalized.contains("WORLD"),
            "expected recognizable text from 'HELLO WORLD', got: {text:?}"
        );
    }
}
