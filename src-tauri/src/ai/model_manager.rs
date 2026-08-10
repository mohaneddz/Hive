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

/// ViT classifier for sensitive content, fine-tuned on a safe/NSFW split.
/// The quantized export is a quarter of the size for the same verdict.
pub const NSFW_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/AdamCodd/vit-base-nsfw-detector/resolve/main/onnx/model_quantized.onnx",
        filename: "nsfw.onnx",
    },
];

/// ViT-GPT2 image captioning. The encoder (ViT) turns the image into visual
/// features; the decoder (GPT-2) writes the sentence one token at a time.
///
/// `decoder_model_merged` bundles the with-past and without-past graphs into a
/// single file, which is what makes step-by-step generation practical.
pub const CAPTION_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/Xenova/vit-gpt2-image-captioning/resolve/main/onnx/encoder_model_quantized.onnx",
        filename: "encoder.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/Xenova/vit-gpt2-image-captioning/resolve/main/onnx/decoder_model_merged_quantized.onnx",
        filename: "decoder.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/Xenova/vit-gpt2-image-captioning/resolve/main/vocab.json",
        filename: "vocab.json",
    },
];

/* ------------------------------------------------------------ AI editor -- */
//
// Each editor tool downloads on its own, the first time it is used. Someone who
// only ever enlarges a photo pays 4.9 MB and nothing else.
//
// Every licence here is Apache 2.0 or BSD-3. The obvious picks were all ruled
// out on that basis: RMBG-1.4 and CodeFormer are non-commercial, and every
// Ultralytics YOLO is AGPL, which would pull Hive's own source under AGPL with
// it. See `.idea/AI-Editor.md` for the full comparison.

/// Real-ESRGAN general x4v3 (BSD-3): the compact variant its authors built for
/// real photographs rather than clean benchmark images. Under 5 MB.
pub const UPSCALE_FILES: &[ModelFile] = &[ModelFile {
    url: "https://huggingface.co/Heliosoph/realesrgan-onnx/resolve/main/realesr-general-x4v3.onnx",
    filename: "upscale.onnx",
}];

/// Two cutout models, because they are good at different things. MODNet was
/// trained on people and resolves hair strand by strand; ISNet handles
/// everything else. Which one runs is decided by whether a face was detected.
///
/// MODNet is taken unquantized — 26 MB rather than 6.6. The quantized export
/// crashed the graphics driver outright: DirectML's coverage of quantization
/// operators is incomplete, and where it is missing the failure is a fault in
/// native code, not a refusal. ISNet below was already unquantized and never
/// had the problem, which is what made the pattern visible.
pub const CUTOUT_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/Xenova/modnet/resolve/main/onnx/model.onnx",
        filename: "portrait.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/tomjackson2023/rembg/resolve/main/isnet-general-use.onnx",
        filename: "general.onnx",
    },
];

/// SlimSAM: SAM pruned to a fraction of its size. Split in two on purpose — the
/// encoder runs once per photo, the decoder runs on every click, so selecting
/// costs one small pass rather than a full one.
///
/// Unquantized for the same reason as MODNet: 40 MB instead of 14, against a
/// graphics driver that faults on the quantized build.
pub const SEGMENT_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/Xenova/slimsam-77-uniform/resolve/main/onnx/vision_encoder.onnx",
        filename: "encoder.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/Xenova/slimsam-77-uniform/resolve/main/onnx/prompt_encoder_mask_decoder.onnx",
        filename: "decoder.onnx",
    },
];

/// LaMa (Apache 2.0). Its Fourier convolutions give every layer a view of the
/// whole frame, which is why it can carry a wall or a horizon across a large
/// hole where ordinary inpainters smear.
pub const INPAINT_FILES: &[ModelFile] = &[ModelFile {
    url: "https://huggingface.co/Carve/LaMa-ONNX/resolve/main/lama_fp32.onnx",
    filename: "inpaint.onnx",
}];

/// Stable Diffusion 1.5 **inpainting** — the generative tool.
///
/// Verified to be the real thing rather than an ordinary model pressed into
/// service: its `model_index.json` names `OnnxStableDiffusionInpaintPipeline`,
/// and its UNet takes 9 channels. Several candidates that look right by name
/// declare 4, and cannot mask properly at all.
///
/// The safety checker in the same repo is skipped — 608 MB whose only job is to
/// censor, on a library of your own photos.
///
/// No tokenizer here on purpose. SD 1.5's text encoder is CLIP, and its
/// vocabulary is byte-for-byte the one already downloaded for semantic search:
/// 49,408 entries, same ids. Fetching a second copy would be waste.
pub const GENERATE_FILES: &[ModelFile] = &[
    ModelFile {
        url: "https://huggingface.co/RanaLLC/stable-diffusion-v1-5-inpainting-onnx-fp16/resolve/main/text_encoder/model.onnx",
        filename: "text_encoder.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/RanaLLC/stable-diffusion-v1-5-inpainting-onnx-fp16/resolve/main/vae_encoder/model.onnx",
        filename: "vae_encoder.onnx",
    },
    ModelFile {
        url: "https://huggingface.co/RanaLLC/stable-diffusion-v1-5-inpainting-onnx-fp16/resolve/main/vae_decoder/model.onnx",
        filename: "vae_decoder.onnx",
    },
    // Last because it is the big one: an interrupted download leaves the three
    // small files already in place.
    ModelFile {
        url: "https://huggingface.co/RanaLLC/stable-diffusion-v1-5-inpainting-onnx-fp16/resolve/main/unet/model.onnx",
        filename: "unet.onnx",
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

pub fn nsfw_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("nsfw")
}

pub fn caption_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("caption")
}

pub fn upscale_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("upscale")
}

pub fn cutout_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("cutout")
}

pub fn segment_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("segment")
}

pub fn inpaint_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("inpaint")
}

pub fn generate_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("generate")
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

pub fn nsfw_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&nsfw_dir(app_data_dir), NSFW_FILES)
}

pub fn caption_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&caption_dir(app_data_dir), CAPTION_FILES)
}

pub fn upscale_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&upscale_dir(app_data_dir), UPSCALE_FILES)
}

pub fn cutout_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&cutout_dir(app_data_dir), CUTOUT_FILES)
}

pub fn segment_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&segment_dir(app_data_dir), SEGMENT_FILES)
}

pub fn inpaint_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&inpaint_dir(app_data_dir), INPAINT_FILES)
}

/// Generation also needs CLIP, whose tokenizer it borrows.
pub fn generate_models_ready(app_data_dir: &Path) -> bool {
    models_ready(&generate_dir(app_data_dir), GENERATE_FILES)
        && clip_dir(app_data_dir).join("tokenizer.json").is_file()
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

pub async fn ensure_nsfw_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&nsfw_dir(app_data_dir), NSFW_FILES, on_progress).await
}

pub async fn ensure_caption_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&caption_dir(app_data_dir), CAPTION_FILES, on_progress).await
}

pub async fn ensure_upscale_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&upscale_dir(app_data_dir), UPSCALE_FILES, on_progress).await
}

pub async fn ensure_cutout_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&cutout_dir(app_data_dir), CUTOUT_FILES, on_progress).await
}

pub async fn ensure_segment_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&segment_dir(app_data_dir), SEGMENT_FILES, on_progress).await
}

pub async fn ensure_inpaint_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&inpaint_dir(app_data_dir), INPAINT_FILES, on_progress).await
}

pub async fn ensure_generate_models(app_data_dir: &Path, on_progress: impl FnMut(u64, u64)) -> anyhow::Result<()> {
    ensure_models(&generate_dir(app_data_dir), GENERATE_FILES, on_progress).await
}

