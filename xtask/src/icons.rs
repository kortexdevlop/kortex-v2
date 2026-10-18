//! `cargo xtask icons`: composed tool icons (SVG + PNG), brand icons and Open Graph images.
//!
//! Every tool only provides a 24x24 `glyph.svg`; the final icon (squircle + universe-hue gradient +
//! glyph + soft shadow) and its social image are composed here, so they are always consistent.
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::tools;

/// OKLCH -> sRGB (gamut-clipped), returns `#rrggbb`.
fn oklch(l: f64, c: f64, h_deg: f64) -> String {
    let h = h_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (0.396_337_777_4_f64)
        .mul_add(a, (0.215_803_757_3_f64).mul_add(b, l))
        .powi(3);
    let m_ = (-0.105_561_345_8_f64)
        .mul_add(a, (-0.063_854_172_8_f64).mul_add(b, l))
        .powi(3);
    let s_ = (-0.089_484_177_5_f64)
        .mul_add(a, (-1.291_485_548_0_f64).mul_add(b, l))
        .powi(3);
    let r = 4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_;
    let g = -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_;
    let bl = -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_;
    let enc = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        let v = if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    format!("#{:02x}{:02x}{:02x}", enc(r), enc(g), enc(bl))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Inner markup and root attributes of a glyph `<svg ...>inner</svg>`.
fn split_glyph(svg: &str) -> Option<(String, String)> {
    let start = svg.find("<svg")?;
    let open_end = svg[start..].find('>')? + start;
    let attrs = svg[start + 4..open_end].trim_end_matches('/').to_owned();
    let inner_end = svg.rfind("</svg>")?;
    Some((attrs, svg[open_end + 1..inner_end].to_owned()))
}

fn attr(attrs: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = attrs.find(&key)? + key.len();
    Some(attrs[i..i + attrs[i..].find('"')?].to_owned())
}

/// 128x128 composed icon. `glyph` is the raw glyph.svg, `hue` the universe OKLCH hue.
pub fn compose_icon(glyph: &str, hue: f64) -> Result<String, String> {
    let (attrs, inner) = split_glyph(glyph).ok_or("glyph.svg illisible")?;
    let stroke_w = attr(&attrs, "stroke-width").unwrap_or_else(|| "1.5".into());
    let (top, bottom) = (oklch(0.80, 0.12, hue), oklch(0.58, 0.17, hue + 12.0));
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"128\" height=\"128\" viewBox=\"0 0 128 128\">\
<defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" stop-color=\"{top}\"/><stop offset=\"1\" stop-color=\"{bottom}\"/></linearGradient>\
<linearGradient id=\"s\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0\" stop-color=\"#fff\" stop-opacity=\".35\"/><stop offset=\".5\" stop-color=\"#fff\" stop-opacity=\"0\"/></linearGradient>\
<filter id=\"d\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"150%\"><feDropShadow dx=\"0\" dy=\"4\" stdDeviation=\"4\" flood-color=\"{bottom}\" flood-opacity=\".35\"/></filter></defs>\
<rect x=\"8\" y=\"6\" width=\"112\" height=\"112\" rx=\"28\" fill=\"url(#g)\" filter=\"url(#d)\"/>\
<rect x=\"8\" y=\"6\" width=\"112\" height=\"112\" rx=\"28\" fill=\"url(#s)\"/>\
<g transform=\"translate(34 32) scale(2.5)\" fill=\"none\" stroke=\"#fff\" stroke-width=\"{stroke_w}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</g></svg>"
    ))
}

/// OKLCH hue (degrees) of a `#rrggbb` color, so app icons follow the accent token.
#[must_use]
pub fn hue_of_hex(hex: &str) -> Option<f64> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let channel = |i: usize| {
        u8::from_str_radix(&digits[i..i + 2], 16).ok().map(|v| {
            let c = f64::from(v) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    };
    let (r, g, b) = (channel(0)?, channel(2)?, channel(4)?);
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    Some(bb.atan2(a).to_degrees().rem_euclid(360.0))
}

/// PNG icon set of an application: the glyph on the accent-hued squircle, in the sizes a PWA
/// manifest and iOS expect.
///
/// # Errors
/// The glyph or the hue cannot be used.
pub fn app_icons(glyph: &str, accent_hex: &str) -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let hue = hue_of_hex(accent_hex)
        .ok_or_else(|| format!("couleur d'accent illisible : {accent_hex}"))?;
    let svg = compose_icon(glyph, hue)?;
    [
        ("icon-192.png", 192),
        ("icon-512.png", 512),
        ("apple-touch-icon.png", 180),
    ]
    .into_iter()
    .map(|(name, size)| Ok((name, render_png(&svg, size)?)))
    .collect()
}

/// Greedy word wrap on character count.
fn wrap(text: &str, max: usize, lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > max {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.len() > lines {
        out.truncate(lines);
        if let Some(last) = out.last_mut() {
            last.push('…');
        }
    }
    out
}

const FONT: &str =
    "Inter, 'SF Pro Display', 'Helvetica Neue', Helvetica, Arial, 'DejaVu Sans', sans-serif";

/// 1200x630 Open Graph image.
pub fn compose_og(icon: &str, title: &str, tagline: &str, footer: &str, hue: f64) -> String {
    let glow = oklch(0.85, 0.09, hue);
    let mut text = String::new();
    for (i, l) in wrap(title, 22, 2).iter().enumerate() {
        let _ = write!(
            text,
            "<text x=\"96\" y=\"{}\" font-size=\"68\" font-weight=\"700\" fill=\"#0b0f19\" letter-spacing=\"-1.5\">{}</text>",
            330 + i * 78,
            esc(l)
        );
    }
    let y0 = 330 + wrap(title, 22, 2).len() * 78 + 10;
    for (i, l) in wrap(tagline, 46, 2).iter().enumerate() {
        let _ = write!(
            text,
            "<text x=\"96\" y=\"{}\" font-size=\"32\" fill=\"#5b6472\">{}</text>",
            y0 + i * 44,
            esc(l)
        );
    }
    let icon_inner = icon.replace("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"128\" height=\"128\" viewBox=\"0 0 128 128\">", "").replace("</svg>", "");
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"630\" viewBox=\"0 0 1200 630\" font-family=\"{FONT}\">\
<defs><radialGradient id=\"bg\" cx=\"85%\" cy=\"10%\" r=\"80%\"><stop offset=\"0\" stop-color=\"{glow}\" stop-opacity=\".55\"/><stop offset=\"1\" stop-color=\"#ffffff\" stop-opacity=\"0\"/></radialGradient></defs>\
<rect width=\"1200\" height=\"630\" fill=\"#ffffff\"/><rect width=\"1200\" height=\"630\" fill=\"url(#bg)\"/>\
<g transform=\"translate(86 90) scale(1.5)\">{icon_inner}</g>\
{text}\
<text x=\"96\" y=\"570\" font-size=\"30\" font-weight=\"600\" fill=\"#0a84ff\">kortex.site</text>\
<text x=\"1104\" y=\"570\" font-size=\"26\" text-anchor=\"end\" fill=\"#5b6472\">{}</text></svg>",
        esc(footer)
    )
}

fn footer(lang: &str) -> &'static str {
    if lang == "fr" {
        "100 % local · vos fichiers restent chez vous"
    } else {
        "100% local · your files stay with you"
    }
}

pub(crate) fn render_png(svg: &str, width: u32) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    {
        let db = opt.fontdb_mut();
        db.load_system_fonts();
        if Path::new("design/fonts").exists() {
            db.load_fonts_dir("design/fonts");
        }
    }
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| format!("svg invalide : {e}"))?;
    let size = tree.size();
    let scale = width as f32 / size.width();
    let h = (size.height() * scale).ceil() as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, h).ok_or("pixmap")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

fn write(path: &str, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    if let Some(p) = Path::new(path).parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::write(path, bytes).map_err(|e| format!("{path} : {e}"))
}

fn universe_hues() -> Result<std::collections::BTreeMap<String, f64>, String> {
    let text = fs::read_to_string("brand/universes.toml").map_err(|e| e.to_string())?;
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    Ok(table
        .get("universe")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|u| {
            Some((
                u.get("id")?.as_str()?.to_owned(),
                u.get("hue")?.as_integer()? as f64,
            ))
        })
        .collect())
}

pub fn run() -> Result<(), String> {
    let hues = universe_hues()?;
    let mut count = 0;
    for t in tools::discover()? {
        let c = t
            .contract
            .map_err(|e| format!("{} : {e}", t.dir.display()))?;
        let glyph = fs::read_to_string(t.dir.join("glyph.svg"))
            .map_err(|e| format!("{}/glyph.svg : {e}", t.dir.display()))?;
        let hue = hues.get(&c.tool.universe).copied().unwrap_or(250.0);
        let icon = compose_icon(&glyph, hue)?;
        write(&format!("design/dist/icons/{}.svg", c.tool.id), &icon)?;
        for lang in ["fr", "en"] {
            let og = compose_og(
                &icon,
                &c.tool.name[lang],
                &c.tool.tagline[lang],
                footer(lang),
                hue,
            );
            write(
                &format!("design/dist/og/{lang}/{}.png", c.tool.slug[lang]),
                render_png(&og, 1200)?,
            )?;
        }
        count += 1;
    }
    // Brand: PWA icons + home OG images.
    let mark = fs::read_to_string("brand/logo/kortex-mark.svg").map_err(|e| e.to_string())?;
    let (_, inner) = split_glyph(&mark).ok_or("logo illisible")?;
    let brand_icon = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"512\" height=\"512\" viewBox=\"0 0 64 64\"><defs><linearGradient id=\"g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0\" stop-color=\"#22d3ee\"/><stop offset=\"1\" stop-color=\"#0a84ff\"/></linearGradient></defs><rect width=\"64\" height=\"64\" rx=\"15\" fill=\"url(#g)\"/><g fill=\"none\" stroke=\"#fff\" stroke-width=\"3.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</g></svg>"
    );
    write("design/dist/icons/kortex.svg", &brand_icon)?;
    write(
        "design/dist/icons/kortex-192.png",
        render_png(&brand_icon, 192)?,
    )?;
    write(
        "design/dist/icons/kortex-512.png",
        render_png(&brand_icon, 512)?,
    )?;
    for (lang, tag) in [
        ("fr", "Des outils gratuits. Vos fichiers restent chez vous."),
        ("en", "Free tools. Your files stay with you."),
    ] {
        let og = compose_og(
            &compose_icon(&mark, 250.0).unwrap_or_default(),
            "Kortex",
            tag,
            footer(lang),
            250.0,
        );
        write(
            &format!("design/dist/og/{lang}/home.png"),
            render_png(&og, 1200)?,
        )?;
    }
    println!("✓ icônes et images OG générées ({count} outil(s) + marque)");
    Ok(())
}
