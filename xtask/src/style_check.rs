//! "Tokens only" check for hand-written styles: no literal color, radius or shadow outside
//! `design/tokens`. Covers the design layer, the shell, the apps and the Astro site styles.
use std::fs;
use std::path::{Path, PathBuf};

const COLOR_FUNCTIONS: [&str; 10] = [
    "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(", "color(",
];

/// `value` with every `var(...)` (nested parentheses included) removed.
fn without_vars(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("var(") {
        out.push_str(&rest[..start]);
        let mut depth = 0_usize;
        let mut end = rest.len();
        for (i, c) in rest[start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push(' ');
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn literal_color(value: &str) -> bool {
    let v = value.to_ascii_lowercase();
    let b = v.as_bytes();
    let hex = b.iter().enumerate().any(|(i, &c)| {
        let digits = b[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_hexdigit())
            .count();
        let ends_word = b
            .get(i + 1 + digits)
            .is_none_or(|c| !c.is_ascii_alphanumeric() && !matches!(c, b'-' | b'_'));
        c == b'#'
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
            && matches!(digits, 3 | 4 | 6 | 8)
            && ends_word
    });
    let function = COLOR_FUNCTIONS.iter().any(|f| {
        v.match_indices(f).any(|(i, _)| {
            (i == 0 || !b[i - 1].is_ascii_alphanumeric() && b[i - 1] != b'-')
                && v[i + f.len()..]
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit() || c == '.')
        })
    });
    let named = v
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .any(|w| matches!(w, "white" | "black"));
    hex || function || named
}

/// A non-zero number outside `var(...)` (a literal length, percentage or offset).
fn literal_length(value: &str) -> bool {
    without_vars(value)
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')' | '/' | '*' | '+'))
        .any(|w| {
            let num: String = w
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            !num.is_empty() && num.parse::<f64>().is_ok_and(|n| n != 0.0)
        })
}

/// `css` with comments blanked (line breaks kept, so line numbers stay exact).
fn without_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find("*/")
            .map_or(rest.len(), |e| start + e + 2);
        out.extend(
            rest[start..end]
                .chars()
                .map(|c| if c == '\n' { '\n' } else { ' ' }),
        );
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Issues for a style sheet (`where_` names the file).
#[must_use]
pub fn css_issues(where_: &str, css: &str) -> Vec<String> {
    let css = without_comments(css);
    let mut issues = Vec::new();
    let mut offset = 0;
    for decl in css.split([';', '{', '}']) {
        let lead = decl.len() - decl.trim_start().len();
        let line = css[..offset + lead].matches('\n').count() + 1;
        offset += decl.len() + 1;
        let Some((prop, value)) = decl.split_once(':') else {
            continue;
        };
        let prop = prop.trim().to_ascii_lowercase();
        if prop.is_empty() || prop.contains(char::is_whitespace) || prop.starts_with("--") {
            continue; // a selector such as `a b:hover`, or a custom property definition
        }
        let at = format!("{where_}:{line}");
        if literal_color(&without_vars(value)) {
            issues.push(format!(
                "✗ {at} : couleur en dur dans `{prop}` — utilise les tokens (--k-*)"
            ));
        }
        if prop.ends_with("radius") && literal_length(value) {
            issues.push(format!("✗ {at} : rayon en dur — utilise --k-radius-*"));
        }
        if matches!(prop.as_str(), "box-shadow" | "text-shadow") && literal_length(value) {
            issues.push(format!("✗ {at} : ombre en dur — utilise --k-shadow-*"));
        }
    }
    issues
}

/// `<style>` blocks and `style="…"` / ``style={`…`}`` attributes of an Astro component.
fn astro_styles(text: &str) -> String {
    let mut out = String::new();
    let mut in_style = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<style") {
            in_style = true;
        }
        if in_style {
            out.push_str(line);
        } else {
            for (marker, close) in [("style=\"", '"'), ("style={`", '`')] {
                for (i, _) in line.match_indices(marker) {
                    let rest = &line[i + marker.len()..];
                    out.push_str(&rest[..rest.find(close).unwrap_or(rest.len())]);
                    out.push(';');
                }
            }
        }
        if trimmed.contains("</style>") {
            in_style = false;
        }
        out.push('\n');
    }
    out
}

fn collect(dir: &Path, ext: &str, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, ext, found);
        } else if path.extension().is_some_and(|e| e == ext) {
            found.push(path);
        }
    }
}

/// Every hand-written style file of the product.
pub fn run() -> Vec<String> {
    let mut files = vec![
        PathBuf::from("design/components.css"),
        PathBuf::from("crates/kortex-shell/src/shell.css"),
    ];
    collect(Path::new("apps"), "css", &mut files);
    collect(Path::new("crates/kortex-ui/src"), "css", &mut files);
    let mut astro = Vec::new();
    collect(Path::new("web/site/src"), "astro", &mut astro);
    let mut issues = Vec::new();
    for path in files.iter().chain(&astro) {
        let Ok(text) = fs::read_to_string(path) else {
            issues.push(format!("✗ {} illisible", path.display()));
            continue;
        };
        let css = if path.extension().is_some_and(|e| e == "astro") {
            astro_styles(&text)
        } else {
            text
        };
        issues.extend(css_issues(&path.display().to_string(), &css));
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::{astro_styles, css_issues};

    #[test]
    fn token_values_pass() {
        let css = ".a{color:var(--k-text);border-radius:var(--k-radius-card);box-shadow:var(--k-shadow-inset),var(--k-shadow-2);background:color-mix(in oklab,var(--k-accent) 20%,transparent)}\n.b:hover{border-radius:0;box-shadow:none}";
        assert_eq!(css_issues("x", css), Vec::<String>::new());
    }

    #[test]
    fn literal_colors_radii_and_shadows_fail() {
        for css in [
            ".a{color:#fff}",
            ".a{background:rgb(0 0 0 / .4)}",
            ".a{fill:oklch(0.7 0.1 200)}",
            ".a{color:white}",
            ".a{border-radius:8px}",
            ".a{border-top-left-radius:50%}",
            ".a{box-shadow:0 0 0 2px var(--k-accent)}",
        ] {
            assert_eq!(css_issues("x", css).len(), 1, "{css}");
        }
    }

    #[test]
    fn selectors_ids_and_token_definitions_are_ignored() {
        let css = "a:hover #main{color:var(--k-text)}\n:root{--k-x:#fff}\n.a{background:oklch(var(--k-hue-l) var(--k-hue-c) var(--hue))}";
        assert_eq!(css_issues("x", css), Vec::<String>::new());
    }

    #[test]
    fn astro_style_blocks_and_attributes_are_checked() {
        let astro = "<div style=\"border-radius:6px\"></div>\n<p style={`color:${x}`}></p>\n<style>\n.a{color:#000}\n</style>";
        assert_eq!(css_issues("x", &astro_styles(astro)).len(), 2);
    }
}
