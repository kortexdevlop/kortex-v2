//! Batch image conversion between PNG, JPG, WebP (lossless), BMP and GIF.
//! Input format is detected from the file signature, never from the extension.
use std::io::Cursor;

use image::codecs::{
    bmp::BmpEncoder, gif::GifEncoder, jpeg::JpegEncoder, png::PngEncoder, webp::WebPEncoder,
};
use image::{ExtendedColorType, ImageEncoder, ImageReader, Limits, RgbaImage};
use kortex::runtime::{Cx, FileData, ToolError};

use crate::{Inputs, Outputs, Params};

const MAX_SIDE: u32 = 20_000;

fn parse_color(s: &str) -> Result<[u8; 3], ToolError> {
    let h = s.trim().strip_prefix('#').unwrap_or(s.trim());
    let bad = || ToolError::new("invalid_color");
    if h.len() != 6 || !h.is_ascii() {
        return Err(bad());
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| bad());
    Ok([b(0)?, b(2)?, b(4)?])
}

fn decode(file: &FileData) -> Result<RgbaImage, ToolError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    let mut reader = ImageReader::new(Cursor::new(&file.bytes))
        .with_guessed_format()
        .map_err(|_| ToolError::with_detail("invalid_image", file.name.clone()))?;
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|_| ToolError::with_detail("invalid_image", file.name.clone()))
}

/// Composite onto an opaque background (for formats without alpha).
fn flatten(img: &RgbaImage, bg: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(img.pixels().len() * 3);
    for p in img.pixels() {
        let a = u32::from(p.0[3]);
        for c in 0..3 {
            let v = (u32::from(p.0[c]) * a + u32::from(bg[c]) * (255 - a) + 127) / 255;
            out.push(u8::try_from(v).unwrap_or(255));
        }
    }
    out
}

fn encode(
    img: &RgbaImage,
    params: &Params,
    bg: [u8; 3],
) -> Result<(Vec<u8>, &'static str, &'static str), ToolError> {
    let (w, h) = img.dimensions();
    let fail = |_| ToolError::new("encode_failed");
    let mut out = Vec::new();
    match params.format.as_str() {
        "png" => {
            PngEncoder::new(&mut out)
                .write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(fail)?;
            Ok((out, "png", "image/png"))
        }
        "jpg" => {
            let quality = u8::try_from(params.quality)
                .map_err(|_| ToolError::new(ToolError::INVALID_PARAM))?;
            JpegEncoder::new_with_quality(&mut out, quality)
                .write_image(&flatten(img, bg), w, h, ExtendedColorType::Rgb8)
                .map_err(fail)?;
            Ok((out, "jpg", "image/jpeg"))
        }
        "webp" => {
            WebPEncoder::new_lossless(&mut out)
                .write_image(img.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(fail)?;
            Ok((out, "webp", "image/webp"))
        }
        "bmp" => {
            BmpEncoder::new(&mut out)
                .write_image(&flatten(img, bg), w, h, ExtendedColorType::Rgb8)
                .map_err(fail)?;
            Ok((out, "bmp", "image/bmp"))
        }
        "gif" => {
            GifEncoder::new(&mut out)
                .encode(img.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(fail)?;
            Ok((out, "gif", "image/gif"))
        }
        _ => Err(ToolError::with_detail(ToolError::INVALID_PARAM, "format")),
    }
}

/// `photo.JPG` -> `photo.png`; names without extension get one appended.
fn output_name(name: &str, ext: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    format!("{}.{ext}", if stem.is_empty() { "image" } else { stem })
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    if input.images.is_empty() {
        return Err(ToolError::new("missing_input"));
    }
    let bg = parse_color(&params.background)?;
    let total = input.images.len();
    let mut converted = Vec::with_capacity(total);
    for (i, file) in input.images.iter().enumerate() {
        cx.check_cancelled()?;
        let img = decode(file)?;
        let (bytes, ext, mime) = encode(&img, &params, bg)?;
        converted.push(FileData {
            name: output_name(&file.name, ext),
            mime: mime.to_owned(),
            bytes,
        });
        #[allow(clippy::cast_precision_loss)]
        cx.progress((i + 1) as f32 / total as f32);
    }
    Ok(Outputs { converted })
}
