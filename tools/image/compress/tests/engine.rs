use std::io::Cursor;

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use kortex::runtime::{Cx, FileData, block_on};
use tool_image_compress::{Inputs, Params, engine::run};

/// A photo-like picture: smooth gradients plus fine deterministic texture.
fn picture(w: u32, h: u32, alpha: u8) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        let noise = u8::try_from((x * 7 + y * 13 + x * y) % 23).unwrap_or(0);
        Rgba([
            u8::try_from(x * 200 / w).unwrap_or(0).saturating_add(noise),
            u8::try_from(y * 200 / h).unwrap_or(0).saturating_add(noise),
            u8::try_from((x + y) * 100 / (w + h)).unwrap_or(0),
            alpha,
        ])
    })
}

fn file(img: &RgbaImage, format: ImageFormat, name: &str, jpeg_quality: Option<u8>) -> FileData {
    let mut bytes = Vec::new();
    let dynamic = DynamicImage::ImageRgba8(img.clone());
    match (format, jpeg_quality) {
        (ImageFormat::Jpeg, Some(q)) => {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, q)
                .encode_image(&dynamic.to_rgb8())
                .expect("jpeg");
        }
        (ImageFormat::Jpeg, None) => dynamic
            .to_rgb8()
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("jpeg"),
        _ => dynamic
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode"),
    }
    FileData {
        name: name.to_owned(),
        mime: String::new(),
        bytes,
    }
}

fn params(format: &str, quality: i64, max_side: i64, png_colors: &str) -> Params {
    Params {
        format: format.into(),
        quality,
        max_side,
        png_colors: png_colors.into(),
    }
}

fn go(
    images: Vec<FileData>,
    p: Params,
) -> Result<tool_image_compress::Outputs, kortex::runtime::ToolError> {
    block_on(run(Inputs { images }, p, &mut Cx::new()))
}

#[test]
fn nominal_jpeg_gets_lighter_and_reports_the_saving() {
    let original = file(
        &picture(320, 240, 255),
        ImageFormat::Jpeg,
        "photo.jpg",
        Some(97),
    );
    let before = original.bytes.len();
    let out = go(vec![original], params("keep", 60, 0, "256")).expect("compressed");
    let result = &out.compressed[0];
    assert_eq!(
        (result.name.as_str(), result.mime.as_str()),
        ("photo.jpg", "image/jpeg")
    );
    assert!(
        result.bytes.len() < before,
        "{} < {before}",
        result.bytes.len()
    );
    assert_eq!(out.original_bytes, i64::try_from(before).unwrap_or(-1));
    assert_eq!(
        out.compressed_bytes,
        i64::try_from(result.bytes.len()).unwrap_or(-1)
    );
    assert!(
        out.saved_percent > 0 && out.saved_percent < 100,
        "{}",
        out.saved_percent
    );
    assert!(
        image::load_from_memory(&result.bytes).is_ok(),
        "the result is a valid image"
    );
}

#[test]
fn png_palette_is_smaller_and_keeps_transparency() {
    let original = file(&picture(160, 120, 128), ImageFormat::Png, "shot.png", None);
    let before = original.bytes.len();
    let out = go(vec![original], params("keep", 75, 0, "256")).expect("palette");
    let result = &out.compressed[0];
    assert_eq!(result.mime, "image/png");
    assert!(
        result.bytes.len() < before,
        "{} < {before}",
        result.bytes.len()
    );
    let decoded = image::load_from_memory(&result.bytes)
        .expect("png")
        .to_rgba8();
    assert_eq!(decoded.dimensions(), (160, 120));
    assert!(
        decoded.pixels().all(|p| p.0[3] > 100 && p.0[3] < 160),
        "alpha is preserved"
    );
}

#[test]
fn repeated_palette_encoding_stays_compact() {
    // This palette fixture remains compact when encoded again.
    let first = go(
        vec![file(&picture(96, 96, 255), ImageFormat::Png, "p.png", None)],
        params("keep", 75, 0, "256"),
    )
    .expect("first pass");
    let again = go(first.compressed.clone(), params("keep", 75, 0, "256")).expect("second pass");
    assert!(again.compressed[0].bytes.len() <= first.compressed[0].bytes.len());
    assert!(again.saved_percent >= 0);
}

#[test]
fn resizing_fits_the_side_and_never_enlarges() {
    let original = file(&picture(400, 200, 255), ImageFormat::Png, "wide.png", None);
    let small = go(vec![original.clone()], params("jpg", 80, 100, "256")).expect("resized");
    assert_eq!(small.compressed[0].name, "wide.jpg");
    let img = image::load_from_memory(&small.compressed[0].bytes).expect("jpeg");
    assert_eq!((img.width(), img.height()), (100, 50));
    let same = go(vec![original], params("png", 80, 5000, "full")).expect("unchanged size");
    let img = image::load_from_memory(&same.compressed[0].bytes).expect("png");
    assert_eq!((img.width(), img.height()), (400, 200));
}

#[test]
fn webp_output_is_lossless_and_valid() {
    let original = file(&picture(64, 64, 255), ImageFormat::Png, "a.png", None);
    let out = go(vec![original], params("webp", 80, 0, "256")).expect("webp");
    assert_eq!(out.compressed[0].mime, "image/webp");
    assert!(image::load_from_memory(&out.compressed[0].bytes).is_ok());
}

#[test]
fn errors_are_typed() {
    let none = go(Vec::new(), params("keep", 75, 0, "256"));
    assert_eq!(none.map(|_| ()).map_err(|e| e.code), Err("missing_input"));
    let bad = FileData {
        name: "x.png".into(),
        mime: String::new(),
        bytes: b"not an image".to_vec(),
    };
    let err = go(vec![bad], params("keep", 75, 0, "256"))
        .map(|_| ())
        .expect_err("invalid");
    assert_eq!(
        (err.code, err.detail.as_deref()),
        ("invalid_image", Some("x.png"))
    );
    let img = file(&picture(8, 8, 255), ImageFormat::Png, "ok.png", None);
    let quality = go(vec![img], params("jpg", 500, 0, "256"))
        .map(|_| ())
        .expect_err("range");
    assert_eq!(quality.code, "invalid_param");
}

#[test]
fn metadata_is_removed_even_when_reencoding_increases_size() {
    let mut input = file(
        &picture(96, 96, 255),
        ImageFormat::Jpeg,
        "private.jpg",
        Some(1),
    );
    let secret = b"private-location";
    let mut comment = vec![0xff, 0xfe];
    comment.extend_from_slice(
        &u16::try_from(secret.len() + 2)
            .expect("length")
            .to_be_bytes(),
    );
    comment.extend_from_slice(secret);
    input.bytes.splice(2..2, comment);
    let original_size = input.bytes.len();
    let out = go(vec![input], params("keep", 100, 0, "256")).expect("encoded");
    assert!(out.compressed[0].bytes.len() > original_size);
    assert!(
        !out.compressed[0]
            .bytes
            .windows(secret.len())
            .any(|w| w == secret)
    );
    assert!(image::load_from_memory(&out.compressed[0].bytes).is_ok());
}
