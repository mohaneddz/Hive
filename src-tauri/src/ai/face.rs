use std::path::Path;

use image::{DynamicImage, GenericImageView};
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;

const DET_W: u32 = 320;
const DET_H: u32 = 240;
const CONF_THRESHOLD: f32 = 0.7;
const IOU_THRESHOLD: f32 = 0.4;
const EMBED_SIZE: u32 = 112;
/// Fraction of the detected box's size added as margin before cropping for embedding/display —
/// UltraFace boxes are drawn tight to facial landmarks, ArcFace expects a bit of headroom.
const CROP_MARGIN: f32 = 0.25;

fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

pub struct DetectedFace {
    pub bbox: (u32, u32, u32, u32),
    pub embedding: Vec<f32>,
}

pub struct FaceModel {
    detector: Session,
    embedder: Session,
}

impl FaceModel {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let detector = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("detector.onnx"))
            .map_err(ort_err)?;
        let embedder = Session::builder()
            .map_err(ort_err)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_err)?
            .with_intra_threads(2)
            .map_err(ort_err)?
            .commit_from_file(dir.join("embedder.onnx"))
            .map_err(ort_err)?;
        Ok(Self { detector, embedder })
    }

    pub fn detect_faces(&mut self, path: &Path) -> anyhow::Result<Vec<DetectedFace>> {
        let img = image::open(path)?;
        let boxes = self.detect_boxes(&img)?;

        let mut faces = Vec::with_capacity(boxes.len());
        for (x0, y0, x1, y1) in boxes {
            let crop = img.crop_imm(x0, y0, x1 - x0, y1 - y0);
            if let Ok(embedding) = self.embed(&crop) {
                faces.push(DetectedFace { bbox: (x0, y0, x1, y1), embedding });
            }
        }
        Ok(faces)
    }

    fn detect_boxes(&mut self, img: &DynamicImage) -> anyhow::Result<Vec<(u32, u32, u32, u32)>> {
        let (orig_w, orig_h) = img.dimensions();
        let resized = img.resize_exact(DET_W, DET_H, image::imageops::FilterType::Triangle);
        let rgb = resized.to_rgb8();

        let (w, h) = (DET_W as usize, DET_H as usize);
        let mut data = vec![0f32; 3 * h * w];
        for (x, y, pixel) in rgb.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                data[c * w * h + y * w + x] = (pixel[c] as f32 - 127.0) / 128.0;
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], data)).map_err(ort_err)?;
        let outputs = self.detector.run(ort::inputs!["input" => tensor]).map_err(ort_err)?;
        let (score_shape, scores) = outputs["scores"].try_extract_tensor::<f32>().map_err(ort_err)?;
        let (_, boxes) = outputs["boxes"].try_extract_tensor::<f32>().map_err(ort_err)?;
        let num_anchors = score_shape[1] as usize;

        let mut candidates: Vec<(f32, [f32; 4])> = Vec::new();
        for i in 0..num_anchors {
            let face_score = scores[i * 2 + 1];
            if face_score > CONF_THRESHOLD {
                let b = [boxes[i * 4], boxes[i * 4 + 1], boxes[i * 4 + 2], boxes[i * 4 + 3]];
                candidates.push((face_score, b));
            }
        }

        let kept = non_max_suppression(candidates, IOU_THRESHOLD);

        let boxes = kept
            .into_iter()
            .filter_map(|(_, b)| {
                let bw = (b[2] - b[0]) * orig_w as f32;
                let bh = (b[3] - b[1]) * orig_h as f32;
                let margin_x = bw * CROP_MARGIN;
                let margin_y = bh * CROP_MARGIN;

                let x0 = ((b[0] * orig_w as f32) - margin_x).max(0.0) as u32;
                let y0 = ((b[1] * orig_h as f32) - margin_y).max(0.0) as u32;
                let x1 = ((b[2] * orig_w as f32) + margin_x).min(orig_w as f32) as u32;
                let y1 = ((b[3] * orig_h as f32) + margin_y).min(orig_h as f32) as u32;

                (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
            })
            .collect();

        Ok(boxes)
    }

    fn embed(&mut self, crop: &DynamicImage) -> anyhow::Result<Vec<f32>> {
        let resized = crop.resize_exact(EMBED_SIZE, EMBED_SIZE, image::imageops::FilterType::Triangle);
        let rgb = resized.to_rgb8();

        let (w, h) = (EMBED_SIZE as usize, EMBED_SIZE as usize);
        let mut data = vec![0f32; 3 * h * w];
        for (x, y, pixel) in rgb.enumerate_pixels() {
            let (x, y) = (x as usize, y as usize);
            for c in 0..3 {
                data[c * w * h + y * w + x] = (pixel[c] as f32 - 127.5) / 127.5;
            }
        }

        let tensor = Tensor::from_array(([1usize, 3, h, w], data)).map_err(ort_err)?;
        let outputs = self.embedder.run(ort::inputs!["data" => tensor]).map_err(ort_err)?;
        let (_, embedding) = outputs["fc1"].try_extract_tensor::<f32>().map_err(ort_err)?;
        Ok(l2_normalize(embedding.to_vec()))
    }
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let ix0 = a[0].max(b[0]);
    let iy0 = a[1].max(b[1]);
    let ix1 = a[2].min(b[2]);
    let iy1 = a[3].min(b[3]);
    let inter = (ix1 - ix0).max(0.0) * (iy1 - iy0).max(0.0);
    let area_a = (a[2] - a[0]).max(0.0) * (a[3] - a[1]).max(0.0);
    let area_b = (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

fn non_max_suppression(mut candidates: Vec<(f32, [f32; 4])>, iou_threshold: f32) -> Vec<(f32, [f32; 4])> {
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut kept: Vec<(f32, [f32; 4])> = Vec::new();
    for (score, bbox) in candidates {
        if kept.iter().all(|(_, kb)| iou(kb, &bbox) < iou_threshold) {
            kept.push((score, bbox));
        }
    }
    kept
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
