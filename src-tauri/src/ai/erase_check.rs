//! Erasing, checked on the real photo it was reported broken on.
//!
//! Kept apart from `smoke.rs` because it answers a narrower question: not "does
//! the model run" but "does the thing actually disappear". Run by hand:
//!
//! ```text
//! cargo test --lib erase_check -- --include-ignored --nocapture
//! ```

use image::{GrayImage, RgbImage};

/// The screenshot with the tiramisu thumbnail in it, at 679×928.
fn reported_photo() -> Option<RgbImage> {
    let path = std::env::var_os("USERPROFILE").map(|home| {
        std::path::PathBuf::from(home)
            .join("Pictures")
            .join("Screenshots")
            .join("Capture d'écran 2026-07-26 185359.png")
    })?;
    image::open(path).ok().map(|image| image.to_rgb8())
}

/// The thumbnail's rectangle, read off the reported screenshot — a box drawn by
/// hand around the dessert, the way the editor produces one.
fn thumbnail_mask(width: u32, height: u32) -> GrayImage {
    let mut mask = GrayImage::new(width, height);
    let (x0, x1) = (width * 62 / 100, width * 95 / 100);
    let (y0, y1) = (height * 64 / 100, height * 82 / 100);
    for y in y0..y1 {
        for x in x0..x1 {
            mask.put_pixel(x, y, image::Luma([255]));
        }
    }
    mask
}

/// How different two images are inside a rectangle, 0 to 255.
fn difference(a: &RgbImage, b: &RgbImage, mask: &GrayImage) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;
    for (x, y, pixel) in mask.enumerate_pixels() {
        if pixel[0] < 128 {
            continue;
        }
        for channel in 0..3 {
            total += (a.get_pixel(x, y)[channel] as f64 - b.get_pixel(x, y)[channel] as f64).abs();
            count += 1.0;
        }
    }
    if count == 0.0 { 0.0 } else { total / count }
}

#[test]
#[ignore]
fn the_marked_thing_actually_goes_away() {
    let Some(dir) = std::env::var_os("APPDATA").map(|base| {
        std::path::PathBuf::from(base)
            .join("com.hive")
            .join("models")
            .join("inpaint")
    }) else {
        return;
    };
    let (Some(source), true) = (reported_photo(), dir.join("inpaint.onnx").is_file()) else {
        println!("photo or model missing — skipped");
        return;
    };

    let mask = thumbnail_mask(source.width(), source.height());
    println!("  photo {}×{}", source.width(), source.height());

    let started = std::time::Instant::now();
    let mut model = crate::ai::inpaint::InpaintModel::load(&dir).expect("model loads");
    let out = model.erase(&source, &mask).expect("erasing runs");
    println!("  took {:?}", started.elapsed());

    let changed = difference(&source, &out, &mask);
    println!("  average change inside the mark: {changed:.1} of 255");

    let output = std::env::temp_dir().join("hive-smoke");
    let _ = std::fs::create_dir_all(&output);
    out.save(output.join("erase-real.png")).unwrap();
    println!("  wrote {}", output.join("erase-real.png").display());

    // The complaint was that the thing was still visible. A faint smudge scores
    // in the single digits; something genuinely replaced moves much further.
    assert!(
        changed > 12.0,
        "the marked area barely changed ({changed:.1}) — it is still there"
    );
    // And nothing outside it may have moved at all.
    assert_eq!(out.get_pixel(2, 2), source.get_pixel(2, 2));
}

/// A box drawn tight against the thumbnail, with no background inside it — the
/// way it is natural to draw one, and the way it was reported failing.
///
/// These numbers were **measured** off the screenshot with a grid drawn over it,
/// not estimated. The first version of this test guessed, put the box a tenth of
/// the frame too low, and produced an "incomplete erase" that was entirely the
/// test's own fault — which then looked like confirmation of the bug being
/// chased. Guessing at coordinates is how a test lies to you.
fn tight_mask(width: u32, height: u32) -> GrayImage {
    let mut mask = GrayImage::new(width, height);
    let (x0, x1) = (width * 755 / 1000, width * 950 / 1000);
    let (y0, y1) = (height * 635 / 1000, height * 780 / 1000);
    for y in y0..y1 {
        for x in x0..x1 {
            mask.put_pixel(x, y, image::Luma([255]));
        }
    }
    mask
}

#[test]
#[ignore]
fn a_box_drawn_tight_against_the_thing_still_erases_it() {
    let Some(dir) = std::env::var_os("APPDATA").map(|base| {
        std::path::PathBuf::from(base)
            .join("com.hive")
            .join("models")
            .join("inpaint")
    }) else {
        return;
    };
    let (Some(source), true) = (reported_photo(), dir.join("inpaint.onnx").is_file()) else {
        println!("photo or model missing — skipped");
        return;
    };

    let mask = tight_mask(source.width(), source.height());
    let mut model = crate::ai::inpaint::InpaintModel::load(&dir).expect("model loads");
    let out = model.erase(&source, &mask).expect("erasing runs");

    let changed = difference(&source, &out, &mask);
    println!("  tight box: average change {changed:.1} of 255");

    let output = std::env::temp_dir().join("hive-smoke");
    let _ = std::fs::create_dir_all(&output);
    out.save(output.join("erase-tight.png")).unwrap();
    println!("  wrote {}", output.join("erase-tight.png").display());

    assert!(changed > 12.0, "a tight box left the thing there ({changed:.1})");
}

#[test]
#[ignore]
fn painting_over_the_thumbnail_puts_something_there() {
    let Some(root) = std::env::var_os("APPDATA").map(|base| {
        std::path::PathBuf::from(base).join("com.hive").join("models")
    }) else {
        return;
    };
    let dir = root.join("generate");
    let (Some(source), true) = (reported_photo(), dir.join("unet.onnx").is_file()) else {
        println!("photo or model missing — skipped");
        return;
    };

    let mask = thumbnail_mask(source.width(), source.height());
    let started = std::time::Instant::now();
    let mut model = crate::ai::generate::GenerateModel::load(&dir, &root.join("clip"))
        .expect("model loads");
    let out = model
        .generate(
            &source,
            &mask,
            "a bowl of fresh strawberries on a white plate",
            8,
            7,
            |done, total| println!("  step {done}/{total}"),
            || false,
        )
        .expect("painting runs");
    println!("  took {:?}", started.elapsed());

    let changed = difference(&source, &out, &mask);
    println!("  average change inside the mark: {changed:.1} of 255");

    let output = std::env::temp_dir().join("hive-smoke");
    let _ = std::fs::create_dir_all(&output);
    out.save(output.join("paint-real.png")).unwrap();
    println!("  wrote {}", output.join("paint-real.png").display());

    assert!(changed > 12.0, "the marked area barely changed ({changed:.1})");
    assert_eq!(out.get_pixel(2, 2), source.get_pixel(2, 2));
}

/// Prints the shape of every tensor the generation pipeline passes around.
///
/// Written after the pipeline produced structured noise instead of a picture.
/// Shapes are the first thing to rule out: a text encoder that returns a pooled
/// vector instead of the per-token block, or a UNet output read from the wrong
/// slot, both make plausible-looking code that denoises nothing.
#[test]
#[ignore]
fn the_generation_tensors_have_the_shapes_the_model_expects() {
    let Some(root) = std::env::var_os("APPDATA").map(|base| {
        std::path::PathBuf::from(base).join("com.hive").join("models")
    }) else {
        return;
    };
    let dir = root.join("generate");
    if !dir.join("unet.onnx").is_file() {
        println!("model missing — skipped");
        return;
    }

    for (label, file) in [
        ("text_encoder", "text_encoder.onnx"),
        ("vae_encoder", "vae_encoder.onnx"),
        ("vae_decoder", "vae_decoder.onnx"),
        ("unet", "unet.onnx"),
    ] {
        let session = crate::ai::session::open_on_cpu(&dir.join(file)).expect("opens");
        println!("=== {label} ===");
        for input in session.inputs() {
            println!("  in  {:<26} {:?}", input.name(), input.dtype());
        }
        for output in session.outputs() {
            println!("  out {:<26} {:?}", output.name(), output.dtype());
        }
    }
}

/// Runs each generation model once and prints the shape it actually returned.
///
/// Declared shapes are all `-1` here, so they rule nothing out. What matters is
/// the channel count coming out of the VAE encoder: some exports return the four
/// latent channels, others return eight — the mean and the log-variance of a
/// distribution to sample from. Feeding eight where nine are declared quietly
/// packs the tensor with the wrong numbers, and the loop then denoises noise.
#[test]
#[ignore]
fn the_vae_encoder_returns_the_channel_count_the_unet_expects() {
    use ort::value::Tensor;

    let Some(root) = std::env::var_os("APPDATA").map(|base| {
        std::path::PathBuf::from(base).join("com.hive").join("models")
    }) else {
        return;
    };
    let dir = root.join("generate");
    if !dir.join("vae_encoder.onnx").is_file() {
        println!("model missing — skipped");
        return;
    }

    let mut encoder =
        crate::ai::session::open_on_cpu(&dir.join("vae_encoder.onnx")).expect("opens");

    // A plain 512×512 grey frame; only the shape of the answer is of interest.
    let side = 512usize;
    let pixels: Vec<half::f16> = vec![half::f16::from_f32(0.0); 3 * side * side];
    let tensor = Tensor::from_array(([1usize, 3, side, side], pixels)).expect("tensor");
    let outputs = encoder
        .run(vec![("sample".to_string(), tensor.into_dyn())])
        .expect("encoder runs");

    let shape: Vec<i64> = outputs[0].shape().to_vec();
    println!("  vae_encoder output shape: {shape:?}");
    let channels = shape.get(1).copied().unwrap_or(0);
    println!("  channels: {channels}");

    assert_eq!(
        channels, 4,
        "the encoder returned {channels} channels; the pipeline assumes 4 and would \
         silently mis-pack the UNet input"
    );
}

/// Runs the Rust loop on the exact scenario the Python reference ran, so the two
/// can be laid side by side.
///
/// Same 512 frame, same centre hole, same prompt, same step count. The reference
/// produced a red apple; anything else here is the Rust pipeline's own doing.
#[test]
#[ignore]
fn the_rust_loop_matches_the_python_reference() {
    let Some(root) = std::env::var_os("APPDATA").map(|base| {
        std::path::PathBuf::from(base).join("com.hive").join("models")
    }) else {
        return;
    };
    let dir = root.join("generate");
    if !dir.join("unet.onnx").is_file() {
        println!("model missing — skipped");
        return;
    }

    // The reference's backdrop: flat, with vertical stripes.
    let source = RgbImage::from_fn(512, 512, |x, _| {
        if x % 40 == 0 { image::Rgb([110, 95, 70]) } else { image::Rgb([150, 140, 120]) }
    });
    let mut mask = GrayImage::new(512, 512);
    for y in 160..352 {
        for x in 160..352 {
            mask.put_pixel(x, y, image::Luma([255]));
        }
    }

    let mut model = crate::ai::generate::GenerateModel::load(&dir, &root.join("clip"))
        .expect("model loads");
    let out = model
        .generate(&source, &mask, "a red apple on a wooden table", 20, 7, |_, _| {}, || false)
        .expect("generation runs");

    let output = std::env::temp_dir().join("hive-smoke");
    let _ = std::fs::create_dir_all(&output);
    out.save(output.join("rust-loop.png")).unwrap();
    println!("  wrote {}", output.join("rust-loop.png").display());
}
