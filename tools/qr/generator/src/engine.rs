//! QR code as PNG (1-bit palette, hand-written encoder) or SVG.
use kortex::runtime::{Cx, FileData, ToolError};
use qrcodegen::{QrCode, QrCodeEcc};

use crate::{Inputs, Outputs, Params};

fn parse_color(s: &str) -> Result<[u8; 3], ToolError> {
    let h = s.trim().strip_prefix('#').unwrap_or(s.trim());
    let bad = || ToolError::new("invalid_color");
    if h.len() != 6 || !h.is_ascii() {
        return Err(bad());
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| bad());
    Ok([b(0)?, b(2)?, b(4)?])
}

fn css(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// `modules[y][x]` dark? `scale` pixels per module, `margin` modules around.
fn png(qr: &QrCode, scale: u32, margin: u32, fg: [u8; 3], bg: [u8; 3]) -> Vec<u8> {
    let n = u32::try_from(qr.size()).unwrap_or(0);
    let dim = (n + 2 * margin) * scale;
    let row_bytes = dim.div_ceil(8) as usize;
    let mut raw = Vec::with_capacity((row_bytes + 1) * dim as usize);
    for py in 0..dim {
        raw.push(0); // filter: none
        let my = i64::from(py / scale) - i64::from(margin);
        let mut row = vec![0_u8; row_bytes]; // palette index 0 = background
        for px in 0..dim {
            let mx = i64::from(px / scale) - i64::from(margin);
            if qr.get_module(
                i32::try_from(mx).unwrap_or(-1),
                i32::try_from(my).unwrap_or(-1),
            ) {
                row[(px / 8) as usize] |= 0x80 >> (px % 8);
            }
        }
        raw.extend_from_slice(&row);
    }
    // zlib stream with stored (uncompressed) deflate blocks.
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = u16::try_from(b.len()).unwrap_or(0);
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&dim.to_be_bytes());
    ihdr.extend_from_slice(&dim.to_be_bytes());
    ihdr.extend_from_slice(&[1, 3, 0, 0, 0]); // 1-bit, palette, deflate, no filter, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(
        &mut out,
        b"PLTE",
        &[bg[0], bg[1], bg[2], fg[0], fg[1], fg[2]],
    );
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn svg(qr: &QrCode, margin: i32, fg: [u8; 3], bg: [u8; 3]) -> String {
    use std::fmt::Write as _;
    let n = qr.size();
    let dim = n + 2 * margin;
    let mut d = String::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            if qr.get_module(x, y) {
                let start = x;
                while x < n && qr.get_module(x, y) {
                    x += 1;
                }
                let _ = write!(
                    d,
                    "M{},{}h{}v1h-{}z",
                    start + margin,
                    y + margin,
                    x - start,
                    x - start
                );
            } else {
                x += 1;
            }
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {dim} {dim}\" shape-rendering=\"crispEdges\"><rect width=\"{dim}\" height=\"{dim}\" fill=\"{}\"/><path d=\"{d}\" fill=\"{}\"/></svg>",
        css(bg),
        css(fg)
    )
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    cx.check_cancelled()?;
    if input.content.is_empty() {
        return Err(ToolError::new("empty_content"));
    }
    let ecc = match params.error_correction.as_str() {
        "L" => QrCodeEcc::Low,
        "M" => QrCodeEcc::Medium,
        "Q" => QrCodeEcc::Quartile,
        "H" => QrCodeEcc::High,
        _ => {
            return Err(ToolError::with_detail(
                ToolError::INVALID_PARAM,
                "error_correction",
            ));
        }
    };
    let (fg, bg) = (
        parse_color(&params.foreground)?,
        parse_color(&params.background)?,
    );
    let qr =
        QrCode::encode_text(&input.content, ecc).map_err(|_| ToolError::new("content_too_long"))?;
    cx.progress(0.5);
    let margin =
        u32::try_from(params.margin).map_err(|_| ToolError::new(ToolError::INVALID_PARAM))?;
    let file = match params.format.as_str() {
        "svg" => FileData {
            name: "qrcode.svg".to_owned(),
            mime: "image/svg+xml".to_owned(),
            bytes: svg(&qr, i32::try_from(margin).unwrap_or(4), fg, bg).into_bytes(),
        },
        "png" => {
            let modules = u32::try_from(qr.size()).unwrap_or(1) + 2 * margin;
            let size =
                u32::try_from(params.size).map_err(|_| ToolError::new(ToolError::INVALID_PARAM))?;
            let scale = (size / modules).max(1);
            FileData {
                name: "qrcode.png".to_owned(),
                mime: "image/png".to_owned(),
                bytes: png(&qr, scale, margin, fg, bg),
            }
        }
        _ => return Err(ToolError::with_detail(ToolError::INVALID_PARAM, "format")),
    };
    Ok(Outputs { image: file })
}
