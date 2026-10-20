//! Opaque sRGB, HSL, OKLCH and WCAG contrast math. No platform access or dependencies.
#[derive(Debug, Clone, Copy)]
pub struct Color(pub [f64; 3]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorError;

fn linear(x: f64) -> f64 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}
fn gamma(x: f64) -> f64 {
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

impl Color {
    /// Parse opaque HEX, RGB (0–255), HSL (degrees, percentages) or OKLCH.
    /// # Errors
    /// Unsupported syntax, non-finite numbers or invalid component ranges.
    pub fn parse(format: &str, value: &str) -> Result<Self, ColorError> {
        let value = value.trim();
        if format == "hex" {
            let hex = value.strip_prefix('#').unwrap_or(value);
            if !hex.is_ascii() || ![3, 6].contains(&hex.len()) {
                return Err(ColorError);
            }
            let hex = if hex.len() == 3 {
                hex.chars().flat_map(|c| [c, c]).collect::<String>()
            } else {
                hex.to_owned()
            };
            let mut rgb = [0.0; 3];
            for (i, c) in rgb.iter_mut().enumerate() {
                *c = f64::from(
                    u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| ColorError)?,
                ) / 255.0;
            }
            return Ok(Self(rgb));
        }
        let body = if let Some(rest) = value.strip_prefix(format).and_then(|v| v.strip_prefix('('))
        {
            rest.strip_suffix(')').ok_or(ColorError)?
        } else {
            value
        };
        let parts: Vec<_> = body
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() != 3 {
            return Err(ColorError);
        }
        let mut n = [0.0; 3];
        for (i, p) in parts.iter().enumerate() {
            n[i] = p
                .trim_end_matches('%')
                .trim_end_matches("deg")
                .parse::<f64>()
                .map_err(|_| ColorError)?;
            if !n[i].is_finite() {
                return Err(ColorError);
            }
        }
        match format {
            "rgb"
                if n.iter().all(|x| (0.0..=255.0).contains(x))
                    && !parts.iter().any(|p| p.ends_with('%')) =>
            {
                Ok(Self(n.map(|x| x / 255.0)))
            }
            "hsl" if (0.0..=100.0).contains(&n[1]) && (0.0..=100.0).contains(&n[2]) => {
                let h = n[0].rem_euclid(360.0) / 60.0;
                let s = n[1] / 100.0;
                let l = n[2] / 100.0;
                let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
                let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
                let m = l - c / 2.0;
                let rgb = match h as u8 {
                    0 => [c, x, 0.0],
                    1 => [x, c, 0.0],
                    2 => [0.0, c, x],
                    3 => [0.0, x, c],
                    4 => [x, 0.0, c],
                    _ => [c, 0.0, x],
                };
                Ok(Self(rgb.map(|v| v + m)))
            }
            "oklch" => {
                let l = if parts[0].ends_with('%') {
                    n[0] / 100.0
                } else {
                    n[0]
                };
                if !(0.0..=1.0).contains(&l)
                    || !(0.0..=0.5).contains(&n[1])
                    || parts[1].ends_with('%')
                {
                    return Err(ColorError);
                }
                let a = n[1] * n[2].to_radians().cos();
                let b = n[1] * n[2].to_radians().sin();
                let ll = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
                let mm = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
                let ss = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
                Ok(Self([
                    gamma(4.0767416621 * ll - 3.3077115913 * mm + 0.2309699292 * ss),
                    gamma(-1.2684380046 * ll + 2.6097574011 * mm - 0.3413193965 * ss),
                    gamma(-0.0041960863 * ll - 0.7034186147 * mm + 1.7076147010 * ss),
                ]))
            }
            _ => Err(ColorError),
        }
    }
    #[must_use]
    pub fn in_gamut(self) -> bool {
        self.0.iter().all(|v| (-0.000001..=1.000001).contains(v))
    }
    #[must_use]
    pub fn clipped(self) -> Self {
        Self(self.0.map(|v| v.clamp(0.0, 1.0)))
    }
    #[must_use]
    pub fn hex(self) -> String {
        let [r, g, b] = self.clipped().0.map(|x| (x * 255.0).round() as u8);
        format!("#{r:02X}{g:02X}{b:02X}")
    }
    #[must_use]
    pub fn rgb(self) -> String {
        let [r, g, b] = self.clipped().0.map(|x| (x * 255.0).round() as u8);
        format!("rgb({r} {g} {b})")
    }
    #[must_use]
    pub fn hsl(self) -> String {
        let [r, g, b] = self.clipped().0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let l = (max + min) / 2.0;
        let (h, s) = if d < 1e-12 {
            (0.0, 0.0)
        } else {
            let h = if max == r {
                ((g - b) / d).rem_euclid(6.0)
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h * 60.0, d / (1.0 - (2.0 * l - 1.0).abs()))
        };
        format!("hsl({h:.2} {:.2}% {:.2}%)", s * 100.0, l * 100.0)
    }
    #[must_use]
    pub fn oklch(self) -> String {
        let [r, g, b] = self.clipped().0.map(linear);
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        let light = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
        let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
        let b = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
        let c = a.hypot(b);
        let h = if c < 1e-7 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        };
        format!("oklch({light:.6} {c:.6} {h:.4})")
    }
    #[must_use]
    pub fn luminance(self) -> f64 {
        let [r, g, b] = self.clipped().0.map(linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }
    #[must_use]
    pub fn contrast(self, other: Self) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_colors_and_formats() {
        let red = Color::parse("hex", "#f00").expect("red");
        assert_eq!(red.hex(), "#FF0000");
        assert_eq!(red.rgb(), "rgb(255 0 0)");
        assert_eq!(
            Color::parse("hsl", "hsl(-360 100% 50%)")
                .expect("red")
                .hex(),
            red.hex()
        );
        let oklch = red.oklch();
        assert!(oklch.starts_with("oklch(0.627955 0.257683"));
        assert_eq!(
            Color::parse("oklch", &oklch).expect("roundtrip").hex(),
            red.hex()
        );
    }
    #[test]
    fn roundtrips_and_wcag_reference() {
        for hex in ["#000000", "#FFFFFF", "#808080", "#0080FF", "#28A745"] {
            let c = Color::parse("hex", hex).expect("color");
            assert_eq!(Color::parse("hsl", &c.hsl()).expect("hsl").hex(), hex);
            assert_eq!(Color::parse("oklch", &c.oklch()).expect("oklch").hex(), hex);
            assert!((c.contrast(c) - 1.0).abs() < 1e-10);
        }
        let black = Color::parse("hex", "000").expect("black");
        let white = Color::parse("hex", "fff").expect("white");
        assert!((black.contrast(white) - 21.0).abs() < 1e-10);
        assert!((Color::parse("hex", "f00").expect("red").luminance() - 0.2126).abs() < 1e-10);
    }
    #[test]
    fn errors_and_gamut() {
        for (kind, value) in [
            ("hex", "ééé"),
            ("hex", "abcd"),
            ("rgb", "256 0 0"),
            ("rgb", "NaN 0 0"),
            ("hsl", "0 101 50"),
            ("oklch", "2 0 0"),
            ("rgb", "0 0 0 / 1"),
        ] {
            assert!(Color::parse(kind, value).is_err());
        }
        assert!(
            !Color::parse("oklch", "0.7 0.4 30")
                .expect("color")
                .in_gamut()
        );
    }
}
