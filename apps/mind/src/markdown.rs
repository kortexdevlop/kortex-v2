//! A small, safe Markdown subset for assistant answers: paragraphs, headings, lists, fenced code,
//! `**bold**`, `*italic*`, `` `code` `` and `[text](https://link)`. It produces a tree of values,
//! never HTML, so nothing the model writes can inject markup or script.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    /// Only `http`/`https` links survive; any other scheme is shown as plain text.
    Link {
        text: String,
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    List(Vec<Vec<Inline>>),
    Code { lang: String, text: String },
}

fn safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && !lower.contains(char::is_whitespace)
}

/// Inline spans of one line of text.
#[must_use]
pub fn inlines(text: &str) -> Vec<Inline> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Inline> = Vec::new();
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Inline>| {
        if !plain.is_empty() {
            out.push(Inline::Text(std::mem::take(plain)));
        }
    };
    let find = |from: usize, pattern: &[char]| -> Option<usize> {
        (from..chars.len().saturating_sub(pattern.len() - 1))
            .find(|&i| chars[i..].starts_with(pattern))
    };
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&['*', '*']) {
            if let Some(end) = find(i + 2, &['*', '*']).filter(|&e| e > i + 2) {
                flush(&mut plain, &mut out);
                out.push(Inline::Bold(chars[i + 2..end].iter().collect()));
                i = end + 2;
                continue;
            }
        } else if chars[i] == '`' {
            if let Some(end) = find(i + 1, &['`']).filter(|&e| e > i + 1) {
                flush(&mut plain, &mut out);
                out.push(Inline::Code(chars[i + 1..end].iter().collect()));
                i = end + 1;
                continue;
            }
        } else if chars[i] == '*' && chars.get(i + 1).is_some_and(|c| !c.is_whitespace()) {
            if let Some(end) =
                find(i + 1, &['*']).filter(|&e| e > i + 1 && !chars[e - 1].is_whitespace())
            {
                flush(&mut plain, &mut out);
                out.push(Inline::Italic(chars[i + 1..end].iter().collect()));
                i = end + 1;
                continue;
            }
        } else if chars[i] == '[' {
            let label_end = find(i + 1, &[']']);
            if let Some(le) = label_end.filter(|&le| chars.get(le + 1) == Some(&'('))
                && let Some(ue) = find(le + 2, &[')'])
            {
                let label: String = chars[i + 1..le].iter().collect();
                let url: String = chars[le + 2..ue].iter().collect();
                flush(&mut plain, &mut out);
                if safe_url(&url) {
                    out.push(Inline::Link {
                        text: label,
                        url: url.trim().to_owned(),
                    });
                } else {
                    out.push(Inline::Text(label));
                }
                i = ue + 1;
                continue;
            }
        }
        plain.push(chars[i]);
        i += 1;
    }
    flush(&mut plain, &mut out);
    out
}

fn list_item(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
    {
        return Some(rest);
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    (digits > 0 && trimmed[digits..].starts_with(". ")).then(|| &trimmed[digits + 2..])
}

#[must_use]
pub fn parse(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut items: Vec<Vec<Inline>> = Vec::new();
    let mut lines = text.lines();
    let close =
        |paragraph: &mut Vec<&str>, items: &mut Vec<Vec<Inline>>, blocks: &mut Vec<Block>| {
            if !paragraph.is_empty() {
                blocks.push(Block::Paragraph(inlines(&paragraph.join(" "))));
                paragraph.clear();
            }
            if !items.is_empty() {
                blocks.push(Block::List(std::mem::take(items)));
            }
        };
    while let Some(line) = lines.next() {
        if let Some(fence) = line.trim_start().strip_prefix("```") {
            close(&mut paragraph, &mut items, &mut blocks);
            let mut code = Vec::new();
            for next in lines.by_ref() {
                if next.trim_start().starts_with("```") {
                    break;
                }
                code.push(next);
            }
            blocks.push(Block::Code {
                lang: fence.trim().to_owned(),
                text: code.join("\n"),
            });
        } else if line.trim().is_empty() {
            close(&mut paragraph, &mut items, &mut blocks);
        } else if let Some(item) = list_item(line) {
            if !paragraph.is_empty() {
                close(&mut paragraph, &mut items, &mut blocks);
            }
            items.push(inlines(item));
        } else if let Some(level) = line
            .trim_start()
            .chars()
            .take_while(|c| *c == '#')
            .count()
            .checked_sub(0)
            .filter(|l| (1..=3).contains(l))
        {
            let rest = line.trim_start()[level..].trim_start();
            if line.trim_start().chars().nth(level) == Some(' ') && !rest.is_empty() {
                close(&mut paragraph, &mut items, &mut blocks);
                blocks.push(Block::Heading(
                    u8::try_from(level).unwrap_or(3),
                    inlines(rest),
                ));
            } else {
                paragraph.push(line.trim());
            }
        } else {
            if !items.is_empty() {
                close(&mut paragraph, &mut items, &mut blocks);
            }
            paragraph.push(line.trim());
        }
    }
    close(&mut paragraph, &mut items, &mut blocks);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Inline {
        Inline::Text(s.into())
    }

    #[test]
    fn inline_spans_are_recognised() {
        assert_eq!(
            inlines("un **gras**, un *italique* et du `code` ok"),
            [
                t("un "),
                Inline::Bold("gras".into()),
                t(", un "),
                Inline::Italic("italique".into()),
                t(" et du "),
                Inline::Code("code".into()),
                t(" ok")
            ]
        );
        assert_eq!(
            inlines("2 * 3 = 6 et 4*5"),
            [t("2 * 3 = 6 et 4*5")],
            "a lone or spaced star is not emphasis"
        );
        assert_eq!(inlines("**non fermé"), [t("**non fermé")]);
    }

    #[test]
    fn only_http_links_become_links() {
        assert_eq!(
            inlines("[Kortex](https://kortex.site)"),
            [Inline::Link {
                text: "Kortex".into(),
                url: "https://kortex.site".into()
            }]
        );
        for bad in [
            "[x](javascript:alert(1))",
            "[x](data:text/html,<b>)",
            "[x](//evil.test)",
            "[x](ftp://a.b)",
        ] {
            assert!(
                inlines(bad)
                    .iter()
                    .all(|i| !matches!(i, Inline::Link { .. })),
                "{bad}"
            );
        }
        assert_eq!(
            inlines("[x](javascript:alert(1))")[0],
            t("x"),
            "the label stays, the link goes"
        );
    }

    #[test]
    fn markup_in_the_answer_is_text_not_html() {
        assert_eq!(
            inlines("<script>alert(1)</script> <b>x</b>"),
            [t("<script>alert(1)</script> <b>x</b>")]
        );
    }

    #[test]
    fn blocks_split_on_blank_lines_lists_headings_and_fences() {
        let text = "# Titre\nUne phrase\nsur deux lignes.\n\n- un\n- deux **fort**\n\n1. premier\n2. second\n\n```rust\nlet x = 1;\n\nlet y = 2;\n```\nFin.";
        let blocks = parse(text);
        assert_eq!(blocks[0], Block::Heading(1, vec![t("Titre")]));
        assert_eq!(
            blocks[1],
            Block::Paragraph(vec![t("Une phrase sur deux lignes.")])
        );
        assert_eq!(
            blocks[2],
            Block::List(vec![
                vec![t("un")],
                vec![t("deux "), Inline::Bold("fort".into())]
            ])
        );
        assert_eq!(
            blocks[3],
            Block::List(vec![vec![t("premier")], vec![t("second")]])
        );
        assert_eq!(
            blocks[4],
            Block::Code {
                lang: "rust".into(),
                text: "let x = 1;\n\nlet y = 2;".into()
            }
        );
        assert_eq!(blocks[5], Block::Paragraph(vec![t("Fin.")]));
        assert_eq!(blocks.len(), 6);
    }

    #[test]
    fn an_unterminated_fence_still_yields_its_code() {
        assert_eq!(
            parse("```\nabc"),
            [Block::Code {
                lang: String::new(),
                text: "abc".into()
            }]
        );
        assert_eq!(
            parse("#pasuntitre"),
            [Block::Paragraph(vec![t("#pasuntitre")])]
        );
        assert!(parse("").is_empty());
    }
}
