//! Batch image compression: JPEG quality, PNG palette reduction (256 colours, NeuQuant) and
//! best deflate, lossless WebP, optional downscaling. Metadata (EXIF, GPS) is never copied; the
//! EXIF orientation is applied to the pixels first so photos keep their direction. A result that
//! would be heavier than the original in the same format returns the original bytes unchanged.
use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType as Resize;
use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Limits,
    RgbaImage,
};
use kortex::runtime::{Cx, FileData, ToolError};

use crate::{Inputs, Outputs, Params};

const MAX_SIDE: u32 = 20_000;

struct Decoded {
    image: RgbaImage,
    format: ImageFormat,
}

fn decode(file: &FileData) -> Result<Decoded, ToolError> {
    let bad = || ToolError::with_detail("invalid_image", file.name.clone());
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    let mut reader = ImageReader::new(Cursor::new(&file.bytes))
        .with_guessed_format()
        .map_err(|_| bad())?;
    let format = reader.format().ok_or_else(bad)?;
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| bad())?;
    let orientation = decoder.orientation().map_err(|_| bad())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| bad())?;
    image.apply_orientation(orientation);
    Ok(Decoded {
        image: image.to_rgba8(),
        format,
    })
}

/// Fits the image inside `max_side` × `max_side` (0 keeps the size); never enlarges.
fn fit(image: RgbaImage, max_side: i64) -> RgbaImage {
    let limit = u32::try_from(max_side).unwrap_or(0);
    let (w, h) = image.dimensions();
    if limit == 0 || (w <= limit && h <= limit) {
        return image;
    }
    DynamicImage::ImageRgba8(image)
        .resize(limit, limit, Resize::Lanczos3)
        .to_rgba8()
}

fn opaque(image: &RgbaImage) -> bool {
    image.pixels().all(|p| p.0[3] == 255)
}

/// Composite onto white (JPEG has no transparency).
fn flatten(image: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(image.pixels().len() * 3);
    for p in image.pixels() {
        let a = u32::from(p.0[3]);
        for channel in p.0.iter().take(3) {
            let v = (u32::from(*channel) * a + 255 * (255 - a) + 127) / 255;
            out.push(u8::try_from(v).unwrap_or(255));
        }
    }
    out
}

fn jpeg(image: &RgbaImage, quality: i64) -> Result<Vec<u8>, ToolError> {
    let quality = u8::try_from(quality).map_err(|_| ToolError::new(ToolError::INVALID_PARAM))?;
    let (w, h) = image.dimensions();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality)
        .write_image(&flatten(image), w, h, ExtendedColorType::Rgb8)
        .map_err(|_| ToolError::new("encode_failed"))?;
    Ok(out)
}

fn png_full(image: &RgbaImage) -> Result<Vec<u8>, ToolError> {
    let (w, h) = image.dimensions();
    let mut out = Vec::new();
    let (bytes, color) = if opaque(image) {
        (
            DynamicImage::ImageRgba8(image.clone()).to_rgb8().into_raw(),
            ExtendedColorType::Rgb8,
        )
    } else {
        (image.as_raw().clone(), ExtendedColorType::Rgba8)
    };
    PngEncoder::new_with_quality(&mut out, CompressionType::Best, FilterType::Adaptive)
        .write_image(&bytes, w, h, color)
        .map_err(|_| ToolError::new("encode_failed"))?;
    Ok(out)
}

/// 256-colour indexed PNG (NeuQuant palette, alpha kept in tRNS).
fn png_palette(image: &RgbaImage) -> Result<Vec<u8>, ToolError> {
    let fail = |_| ToolError::new("encode_failed");
    let (w, h) = image.dimensions();
    let quantizer = color_quant::NeuQuant::new(10, 256, image.as_raw());
    let indices: Vec<u8> = image
        .pixels()
        .map(|p| u8::try_from(quantizer.index_of(&p.0)).unwrap_or(0))
        .collect();
    let map = quantizer.color_map_rgba();
    let (mut palette, mut alpha) = (Vec::with_capacity(768), Vec::with_capacity(256));
    for rgba in map.as_chunks::<4>().0 {
        palette.extend_from_slice(&rgba[..3]);
        alpha.push(rgba[3]);
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w, h);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette);
    if alpha.iter().any(|a| *a != 255) {
        encoder.set_trns(alpha);
    }
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().map_err(fail)?;
    writer.write_image_data(&indices).map_err(fail)?;
    writer.finish().map_err(fail)?;
    Ok(out)
}

fn webp(image: &RgbaImage) -> Result<Vec<u8>, ToolError> {
    let (w, h) = image.dimensions();
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .write_image(image.as_raw(), w, h, ExtendedColorType::Rgba8)
        .map_err(|_| ToolError::new("encode_failed"))?;
    Ok(out)
}

/// Output format for `keep`: same family, BMP/GIF/others become PNG (lossless, far smaller).
fn target(format: &str, source: ImageFormat) -> &str {
    match (format, source) {
        ("keep", ImageFormat::Jpeg) => "jpg",
        ("keep", ImageFormat::WebP) => "webp",
        ("keep", _) => "png",
        (other, _) => other,
    }
}

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
    let total = input.images.len();
    let mut compressed = Vec::with_capacity(total);
    let (mut before, mut after) = (0_i64, 0_i64);
    for (i, file) in input.images.iter().enumerate() {
        cx.check_cancelled()?;
        let decoded = decode(file)?;
        let resized = params.max_side > 0;
        let image = fit(decoded.image, params.max_side);
        let ext = target(&params.format, decoded.format);
        let (bytes, mime) = match ext {
            "jpg" => (jpeg(&image, params.quality)?, "image/jpeg"),
            "webp" => (webp(&image)?, "image/webp"),
            "png" if params.png_colors == "256" => (png_palette(&image)?, "image/png"),
            "png" => (png_full(&image)?, "image/png"),
            _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "format")),
        };
        let same = ImageFormat::from_mime_type(mime) == Some(decoded.format);
        // Never heavier than the original in the same format (unless asked to resize).
        let (bytes, mime, name) = if same && !resized && bytes.len() >= file.bytes.len() {
            (file.bytes.clone(), mime, file.name.clone())
        } else {
            (bytes, mime, output_name(&file.name, ext))
        };
        before += i64::try_from(file.bytes.len()).unwrap_or(i64::MAX);
        after += i64::try_from(bytes.len()).unwrap_or(i64::MAX);
        compressed.push(FileData {
            name,
            mime: mime.to_owned(),
            bytes,
        });
        #[allow(clippy::cast_precision_loss)]
        cx.progress((i + 1) as f32 / total as f32);
    }
    let saved_percent = if before > 0 {
        (before - after) * 100 / before
    } else {
        0
    };
    Ok(Outputs {
        compressed,
        original_bytes: before,
        compressed_bytes: after,
        saved_percent,
    })
}
