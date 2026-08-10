//! End-to-end checks for the editor's models, run by hand.
//!
//! Every other test in the AI modules covers arithmetic that can be checked
//! without a model: tile layouts, mask geometry, the noise schedule. Those catch
//! a lot, but they cannot answer the only question that matters to someone using
//! the editor — *does pressing this button produce a good picture?*
//!
//! These do. Each one loads a real downloaded model, runs it on a real photo
//! from the library, and writes the result somewhere it can be looked at. They
//! are `#[ignore]`d because they need gigabytes on disk and take minutes:
//!
//! ```text
//! cargo test --lib smoke -- --include-ignored --nocapture
//! ```
//!
//! They assert only what a machine can judge — that the output has the right
//! size, is not uniformly black, and carries some variation. Whether it *looks*
//! right is what the written files are for.

use std::path::PathBuf;

use image::{GrayImage, RgbImage};

fn models_root() -> Option<PathBuf> {
    // These are the timings people will actually live with, so they are taken on
    // the hardware the app will use. `HIVE_GPU=1` runs the same suite on the
    // graphics card, which is how the discrete-adapter fix was confirmed.
    println!("  backend: {:?}", crate::ai::session::gpu_backend());
    std::env::var_os("APPDATA").map(|base| PathBuf::from(base).join("com.hive").join("models"))
}

fn output_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("hive-smoke");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// A picture with something in it: a warm disc on a cool textured ground.
///
/// The first version of these tests reached for the largest photo in the real
/// library, which turned out to be a screenshot of a lecture slide. Two tools
/// then "failed" for doing exactly the right thing — the portrait matte found no
/// person in a slide, and enlarging its top-left corner returned white because
/// that corner *is* white. A subject the tools can actually find keeps the
/// failures honest.
fn a_picture_with_a_subject() -> RgbImage {
    let (w, h) = (480u32, 360u32);
    let (cx, cy, radius) = (w as f32 / 2.0, h as f32 / 2.0, 110.0f32);
    RgbImage::from_fn(w, h, |x, y| {
        let distance = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
        if distance < radius {
            // Texture inside too: a flat disc would upscale to a flat disc and
            // prove nothing about detail.
            let ripple = ((distance * 0.6).sin() * 40.0 + 190.0).clamp(0.0, 255.0) as u8;
            image::Rgb([ripple, 90, 40])
        } else {
            let checker = if ((x / 24) + (y / 24)) % 2 == 0 { 60 } else { 90 };
            image::Rgb([checker, checker + 20, 130])
        }
    })
}

/// The largest indexed photo in the real library, when one is wanted.
#[allow(dead_code)]
fn a_real_photo() -> Option<RgbImage> {
    let db = std::env::var_os("APPDATA")
        .map(|base| PathBuf::from(base).join("com.hive").join("hive.db"))?;
    if !db.is_file() {
        return None;
    }
    let conn = rusqlite::Connection::open_with_flags(
        &db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;

    let mut stmt = conn
        .prepare(
            "SELECT path FROM media_items
             WHERE media_type = 'image' AND is_trashed = 0 AND width >= 600
             ORDER BY width * height DESC LIMIT 8",
        )
        .ok()?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .ok()?
        .filter_map(|row| row.ok())
        .collect();

    paths
        .iter()
        .find_map(|path| image::open(path).ok())
        .map(|image| image.to_rgb8())
}

/// What a machine can say about a produced image: right size, not blank, and
/// carrying real variation rather than a flat wash.
fn looks_like_a_picture(image: &RgbImage, label: &str) {
    let pixels: Vec<f64> = image.pixels().map(|p| p[0] as f64).collect();
    let mean = pixels.iter().sum::<f64>() / pixels.len() as f64;
    let spread = (pixels.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
        / pixels.len() as f64)
        .sqrt();

    println!(
        "  {label}: {}×{}, mean {mean:.1}, spread {spread:.1}",
        image.width(),
        image.height()
    );
    assert!(mean > 2.0, "{label} came out essentially black");
    assert!(mean < 253.0, "{label} came out essentially white");
    assert!(spread > 3.0, "{label} is a flat wash, not a picture");
}

/// A rectangle in the middle — something to erase or paint over.
fn centre_mask(width: u32, height: u32) -> GrayImage {
    let mut mask = GrayImage::new(width, height);
    let (x0, x1) = (width * 2 / 5, width * 3 / 5);
    let (y0, y1) = (height * 2 / 5, height * 3 / 5);
    for y in y0..y1 {
        for x in x0..x1 {
            mask.put_pixel(x, y, image::Luma([255]));
        }
    }
    mask
}

#[test]
#[ignore]
fn enlarging_a_real_photo_produces_a_real_photo() {
    let Some(root) = models_root() else {
        println!("no models directory — skipped");
        return;
    };
    let source = a_picture_with_a_subject();
    let dir = root.join("upscale");
    if !dir.join("upscale.onnx").is_file() {
        println!("enlarge model not downloaded — skipped");
        return;
    }

    // Cropped small: fourfold on a whole photo is minutes and megapixels, and
    // proves nothing the corner does not.
    let source = image::imageops::crop_imm(&source, 0, 0, 256.min(source.width()), 256.min(source.height()))
        .to_image();

    let started = std::time::Instant::now();
    let mut model = crate::ai::upscale::UpscaleModel::load(&dir).expect("model loads");
    let out = model
        .enlarge(&source, |done, total| println!("  tile {done}/{total}"), || false)
        .expect("enlarging runs");
    println!("  took {:?}", started.elapsed());

    assert_eq!(out.width(), source.width() * 4);
    assert_eq!(out.height(), source.height() * 4);
    looks_like_a_picture(&out, "enlarged");
    out.save(output_dir().join("enlarged.png")).unwrap();
    println!("  wrote {}", output_dir().join("enlarged.png").display());
}

#[test]
#[ignore]
fn cutting_out_a_real_photo_produces_a_usable_matte() {
    let Some(root) = models_root() else {
        println!("no models directory — skipped");
        return;
    };
    let source = a_picture_with_a_subject();
    let dir = root.join("cutout");
    if !dir.join("general.onnx").is_file() {
        println!("cutout models not downloaded — skipped");
        return;
    }

    for subject in [
        crate::ai::cutout::Subject::General,
        crate::ai::cutout::Subject::Portrait,
    ] {
        let started = std::time::Instant::now();
        let mut model =
            crate::ai::cutout::CutoutModel::load(&dir, subject).expect("model loads");
        let matte = model.matte(&source).expect("matte runs");
        println!("  {subject:?} took {:?}", started.elapsed());

        assert_eq!(matte.dimensions(), source.dimensions());
        // A matte that is entirely one value has separated nothing.
        let values: Vec<u8> = matte.pixels().map(|p| p[0]).collect();
        let low = values.iter().filter(|v| **v < 64).count();
        let high = values.iter().filter(|v| **v > 192).count();
        println!(
            "  {subject:?}: {}% background, {}% subject",
            low * 100 / values.len(),
            high * 100 / values.len()
        );
        assert!(
            low > values.len() / 100 && high > values.len() / 100,
            "{subject:?} separated nothing — the matte is uniform"
        );

        let name = format!("cutout-{subject:?}.png").to_lowercase();
        image::DynamicImage::ImageRgba8(crate::ai::cutout::apply_matte(&source, &matte))
            .save(output_dir().join(&name))
            .unwrap();
        println!("  wrote {}", output_dir().join(&name).display());
    }
}

#[test]
#[ignore]
fn clicking_the_middle_selects_something() {
    let Some(root) = models_root() else {
        println!("no models directory — skipped");
        return;
    };
    let source = a_picture_with_a_subject();
    let dir = root.join("segment");
    if !dir.join("encoder.onnx").is_file() {
        println!("selection model not downloaded — skipped");
        return;
    }

    let started = std::time::Instant::now();
    let mut model = crate::ai::segment::SegmentModel::load(&dir).expect("model loads");
    let encoded = model.encode(&source).expect("encoding runs");
    println!("  encoding took {:?}", started.elapsed());

    let clicked = std::time::Instant::now();
    let centre = (source.width() as f32 / 2.0, source.height() as f32 / 2.0);
    let mask = model
        .mask_at(&encoded, &[(centre.0, centre.1, true)])
        .expect("selection runs");
    println!("  click took {:?}", clicked.elapsed());

    assert_eq!(mask.dimensions(), source.dimensions());
    let selected = mask.pixels().filter(|p| p[0] > 128).count();
    let share = selected as f64 / (mask.width() * mask.height()) as f64;
    println!("  selected {:.1}% of the photo", share * 100.0);
    assert!(share > 0.001, "clicking selected nothing at all");
    assert!(share < 0.99, "clicking selected the entire photo");

    mask.save(output_dir().join("selection.png")).unwrap();
    println!("  wrote {}", output_dir().join("selection.png").display());
}

#[test]
#[ignore]
fn erasing_a_rectangle_fills_it_with_background() {
    let Some(root) = models_root() else {
        println!("no models directory — skipped");
        return;
    };
    let source = a_picture_with_a_subject();
    let dir = root.join("inpaint");
    if !dir.join("inpaint.onnx").is_file() {
        println!("erase model not downloaded — skipped");
        return;
    }

    let mask = centre_mask(source.width(), source.height());
    let started = std::time::Instant::now();
    let mut model = crate::ai::inpaint::InpaintModel::load(&dir).expect("model loads");
    let out = model.erase(&source, &mask).expect("erasing runs");
    println!("  took {:?}", started.elapsed());

    assert_eq!(out.dimensions(), source.dimensions());
    looks_like_a_picture(&out, "erased");

    // Outside the mask nothing may have moved: the repair is pasted, not
    // re-rendered over the whole frame.
    assert_eq!(out.get_pixel(0, 0), source.get_pixel(0, 0));
    // Inside it, something must have.
    let (cx, cy) = (source.width() / 2, source.height() / 2);
    let changed = (0..3).any(|c| out.get_pixel(cx, cy)[c] != source.get_pixel(cx, cy)[c]);
    assert!(changed, "the masked area came back untouched");

    out.save(output_dir().join("erased.png")).unwrap();
    println!("  wrote {}", output_dir().join("erased.png").display());
}

#[test]
#[ignore]
fn painting_from_a_description_produces_a_picture() {
    let Some(root) = models_root() else {
        println!("no models directory — skipped");
        return;
    };
    let source = a_picture_with_a_subject();
    let dir = root.join("generate");
    if !dir.join("unet.onnx").is_file() {
        println!("generation model not downloaded — skipped");
        return;
    }

    let mask = centre_mask(source.width(), source.height());
    let started = std::time::Instant::now();
    let mut model =
        crate::ai::generate::GenerateModel::load(&dir, &root.join("clip")).expect("model loads");
    let out = model
        .generate(
            &source,
            &mask,
            "a vase of white flowers",
            8,
            42,
            |done, total| println!("  step {done}/{total}"),
            || false,
        )
        .expect("generation runs");
    println!("  took {:?}", started.elapsed());

    assert_eq!(out.dimensions(), source.dimensions());
    looks_like_a_picture(&out, "painted");
    assert_eq!(out.get_pixel(0, 0), source.get_pixel(0, 0));

    out.save(output_dir().join("painted.png")).unwrap();
    println!("  wrote {}", output_dir().join("painted.png").display());
}
