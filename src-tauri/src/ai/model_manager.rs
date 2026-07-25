use std::path::{Path, PathBuf};

use futures_util::StreamExt;

pub struct ModelFile {
    pub url: &'static str,
    pub filename: &'static str,
}

/// CLIP ViT-B/32 (Xenova ONNX export), uint8-quantized for reasonable download size
/// and CPU inference speed. Vision + text towers are separate ONNX graphs.
pub const CLIP_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model_uint8.onnx",
        filename: "vision_model.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model_uint8.onnx",
        filename: "text_model.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/tokenizer.json",
        filename: "tokenizer.json",
    },
];

/// PaddleOCR PP-OCRv5 detection (server, shared across languages) + English recognition
/// (mobile) ONNX export, plus the CTC character dictionary.
pub const OCR_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/detection/v5/det.onnx",
        filename: "det.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/english/rec.onnx",
        filename: "rec.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/english/dict.txt",
        filename: "dict.txt",
    },
];

/// UltraFace RFB-320 (face detection, ONNX Model Zoo) + ArcFace ResNet100 int8
/// (face embedding, 512-dim), both well-established, widely documented models.
pub const FACE_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://media.githubusercontent.com/media/onnx/models/main/validated/vision/body_analysis/ultraface/models/version-RFB-320.onnx",
        filename: "detector.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/onnxmodelzoo/arcfaceresnet100-11-int8/resolve/main/arcfaceresnet100-11-int8.onnx",
        filename: "embedder.onnx",
    },
];

pub fn clip_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("clip")
}

pub fn ocr_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("ocr")
}

pub fn face_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("faces")
}

fn models_ready(dir: &Path, files: &[ModelFile]) -> bool {
    files.iter().all(|f| dir.join(f.filename).is_file())
}

pub fn clip_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&clip_dir(app_data_dir), CLIP_FILES)
}

pub fn ocr_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&ocr_dir(app_data_dir), OCR_FILES)
}

pub fn face_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&face_dir(app_data_dir), FACE_FILES)
}

/// Downloads any files missing from `dir`, reporting (bytes_done, bytes_total) via `on_progress`.
/// Each file is written to a `.part` path and renamed on success so a partial/interrupted
/// download never looks like a valid model file on the next launch.
pub async fn ensure_models(
    dir: &Path,
    files: &[ModelFile],
    mut on_progress: impl FnMut(u64, u64),
) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(dir).await?;

    let missing: Vec<&ModelFile> = files.iter().filter(|f| !dir.join(f.filename).is_file()).collect();
    if missing.is_empty() {
        return Ok(());
    }

    let client = reqwest::Client::new();
    let mut total_size = 0u64;
    for file in &missing {
        let resp = client.head(file.url).send().await?;
        let size = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        total_size += size;
    }

    let mut done = 0u64;
    for file in &missing {
        let part_path = dir.join(format!("{}.part", file.filename));
        let final_path = dir.join(file.filename);

        let resp = client.get(file.url).send().await?.error_for_status()?;
        let mut stream = resp.bytes_stream();
        let mut out = tokio::fs::File::create(&part_path).await?;
        use tokio::io::AsyncWriteExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            out.write_all(&chunk).await?;
            done += chunk.len() as u64;
            on_progress(done, total_size.max(done));
        }
        out.flush().await?;
        drop(out);
        tokio::fs::rename(&part_path, &final_path).await?;
    }

    Ok(())
}

pub async fn ensure_clip_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&clip_dir(app_data_dir), CLIP_FILES, on_progress).await
}

pub async fn ensure_ocr_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&ocr_dir(app_data_dir), OCR_FILES, on_progress).await
}

pub async fn ensure_face_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&face_dir(app_data_dir), FACE_FILES, on_progress).await
}
