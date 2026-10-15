use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};
use kortex::runtime::{Cx, FileData, block_on};
use tool_image_convert::{Inputs, Params, engine::run};

fn sample(w: u32, h: u32, alpha: u8) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        Rgba([
            u8::try_from(x * 255 / w).unwrap_or(0),
            u8::try_from(y * 255 / h).unwrap_or(0),
            128,
            alpha,
        ])
    })
}

fn encoded(img: &RgbaImage, fmt: ImageFormat, name: &str) -> FileData {
    let mut bytes = Vec::new();
    let dynimg = image::DynamicImage::ImageRgba8(img.clone());
    let dynimg = if fmt == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(dynimg.to_rgb8())
    } else {
        dynimg
    };
    dynimg
        .write_to(&mut Cursor::new(&mut bytes), fmt)
        .expect("encode sample");
    FileData {
        name: name.to_owned(),
        mime: String::new(),
        bytes,
    }
}

fn convert(files: Vec<FileData>, format: &str) -> Result<Vec<FileData>, &'static str> {
    let params = Params {
        format: format.to_owned(),
        ..Params::default()
    };
    block_on(run(Inputs { images: files }, params, &mut Cx::new()))
        .map(|o| o.converted)
        .map_err(|e| e.code)
}

fn decode(f: &FileData) -> (image::ImageFormat, RgbaImage) {
    let fmt = image::guess_format(&f.bytes).expect("format");
    (
        fmt,
        image::load_from_memory(&f.bytes)
            .expect("decodes")
            .to_rgba8(),
    )
}

#[test]
fn png_to_each_format_roundtrips_dimensions_and_extension() {
    let src = sample(40, 30, 255);
    for (fmt, ext, expected) in [
        ("jpg", "jpg", ImageFormat::Jpeg),
        ("webp", "webp", ImageFormat::WebP),
        ("bmp", "bmp", ImageFormat::Bmp),
        ("gif", "gif", ImageFormat::Gif),
        ("png", "png", ImageFormat::Png),
    ] {
        let out =
            convert(vec![encoded(&src, ImageFormat::Png, "photo.PNG")], fmt).expect("converted");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, format!("photo.{ext}"));
        let (got, img) = decode(&out[0]);
        assert_eq!(got, expected);
        assert_eq!(img.dimensions(), (40, 30));
    }
}

#[test]
fn lossless_formats_keep_pixels_exactly() {
    let src = sample(16, 16, 255);
    for fmt in ["png", "webp"] {
        let out = convert(vec![encoded(&src, ImageFormat::Png, "a.png")], fmt).expect("converted");
        assert_eq!(decode(&out[0]).1, src, "{fmt} must be lossless");
    }
}

#[test]
fn transparency_is_flattened_on_background_for_jpg() {
    let src = RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 0])); // fully transparent
    let out = convert(vec![encoded(&src, ImageFormat::Png, "t.png")], "jpg").expect("converted");
    let px = decode(&out[0]).1.get_pixel(4, 4).0;
    assert!(
        px[0] > 240 && px[1] > 240 && px[2] > 240,
        "expected white background, got {px:?}"
    );
}

#[test]
fn batch_keeps_order_and_detects_format_by_signature_not_extension() {
    let png = encoded(
        &sample(10, 10, 255),
        ImageFormat::Png,
        "wrong-extension.jpg",
    );
    let jpg = encoded(&sample(12, 12, 255), ImageFormat::Jpeg, "second.png");
    let out = convert(vec![png, jpg], "webp").expect("converted");
    assert_eq!(
        out.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["wrong-extension.webp", "second.webp"]
    );
    assert_eq!(decode(&out[0]).1.dimensions(), (10, 10));
    assert_eq!(decode(&out[1]).1.dimensions(), (12, 12));
}

#[test]
fn errors_on_no_input_garbage_bad_color_and_bad_quality() {
    assert_eq!(convert(vec![], "png").map(|_| ()), Err("missing_input"));
    let junk = FileData {
        name: "x.png".into(),
        mime: String::new(),
        bytes: b"not an image".to_vec(),
    };
    assert_eq!(convert(vec![junk], "png").map(|_| ()), Err("invalid_image"));
    let ok = encoded(&sample(4, 4, 255), ImageFormat::Png, "a.png");
    let bad_color = Params {
        background: "nope".into(),
        ..Params::default()
    };
    assert_eq!(
        block_on(run(
            Inputs {
                images: vec![ok.clone()]
            },
            bad_color,
            &mut Cx::new()
        ))
        .map(|_| ())
        .map_err(|e| e.code),
        Err("invalid_color")
    );
    let bad_quality = Params {
        quality: 0,
        ..Params::default()
    };
    assert_eq!(
        block_on(run(
            Inputs { images: vec![ok] },
            bad_quality,
            &mut Cx::new()
        ))
        .map(|_| ())
        .map_err(|e| e.code),
        Err("invalid_param")
    );
}
