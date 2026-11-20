use anyhow::{anyhow, Context, Result};
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use std::fs::File;
use std::path::Path;
use std::process::Command;

pub const RAW_EXTENSIONS: &[&str] = &["cr2", "cr3", "nef", "arw", "dng", "orf", "rw2", "raf"];

pub fn requires_extended_decoder(path: &Path) -> bool {
    let ext = extension(path);
    matches!(ext.as_str(), "avif" | "heic" | "heif") || RAW_EXTENSIONS.contains(&ext.as_str())
}

pub fn open(path: &Path) -> Result<DynamicImage> {
    let ext = extension(path);
    let result = if matches!(ext.as_str(), "heic" | "heif") {
        (|| decode(heifer::image_crate::HeifDecoder::new(File::open(path)?)?))()
    } else if RAW_EXTENSIONS.contains(&ext.as_str()) {
        open_raw(path)
    } else {
        (|| {
            decode(
                ImageReader::open(path)?
                    .with_guessed_format()?
                    .into_decoder()?,
            )
        })()
    };

    result.or_else(|native| {
        if !requires_extended_decoder(path) || !crate::thumbnails::ffmpeg_available() {
            return Err(native);
        }
        open_with_ffmpeg(path).with_context(|| format!("Built-in decoder: {native}"))
    })
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn decode(mut decoder: impl ImageDecoder) -> Result<DynamicImage> {
    let mut limits = image::Limits::default();
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn open_raw(path: &Path) -> Result<DynamicImage> {
    use flashraw::{DecodeOptions, FlashRaw, OutputFormat, RawPixelData};

    let raw = FlashRaw::open_path(path)?;
    let frame = raw.decode(DecodeOptions {
        output: OutputFormat::Rgba8,
        ..Default::default()
    })?;
    let RawPixelData::U8(pixels) = frame.data else {
        return Err(anyhow!("RAW decoder returned an unexpected pixel format"));
    };
    RgbaImage::from_raw(frame.width, frame.height, pixels)
        .map(DynamicImage::ImageRgba8)
        .context("Invalid RAW frame dimensions")
}

fn open_with_ffmpeg(path: &Path) -> Result<DynamicImage> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "pipe:1",
        ])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "FFmpeg could not decode this image: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(image::load_from_memory_with_format(
        &output.stdout,
        image::ImageFormat::Png,
    )?)
}
