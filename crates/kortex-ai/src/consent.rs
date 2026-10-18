//! Attachments, what the model may know about them, and quoted text turned into references.
//! Content goes to the model only for an attachment the user explicitly shared (MASTER_PROMPT
//! §9.3, RULES 6.4); the default is metadata: reference, kind, name, type and size.

/// Longest text a user can share with the model (characters).
pub const MAX_SHARED_CHARS: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Text,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// 1-based, per kind: `@texte1`, `@fichier1`.
    pub n: usize,
    pub kind: Kind,
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    /// The user agreed to send this attachment's content to the model.
    pub share_content: bool,
}

impl Attachment {
    #[must_use]
    pub fn text(n: usize, value: &str) -> Self {
        Self {
            n,
            kind: Kind::Text,
            name: format!("texte{n}"),
            mime: "text/plain".into(),
            bytes: value.as_bytes().to_vec(),
            share_content: false,
        }
    }

    #[must_use]
    pub fn reference(&self, lang: &str) -> String {
        let _ = lang;
        match self.kind {
            Kind::Text => format!("@texte{}", self.n),
            Kind::File => format!("@fichier{}", self.n),
        }
    }

    /// The text content, when the attachment is valid UTF-8 text.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes).ok()
    }

    /// What the model is told about this attachment.
    #[must_use]
    pub fn describe(&self, lang: &str) -> String {
        let fr = lang == "fr";
        let size = self
            .as_text()
            .map_or(self.bytes.len(), |t| t.chars().count());
        let unit = match (&self.kind, fr) {
            (Kind::Text, true) => "caractères",
            (Kind::Text, false) => "characters",
            (Kind::File, true) => "octets",
            (Kind::File, false) => "bytes",
        };
        let mut line = format!(
            "{} = {} ({}, {size} {unit})",
            self.reference(lang),
            self.name,
            self.mime
        );
        if self.share_content
            && let Some(text) = self
                .as_text()
                .filter(|t| t.chars().count() <= MAX_SHARED_CHARS)
        {
            line.push_str(if fr {
                " — contenu partagé par l'utilisateur : "
            } else {
                " — content shared by the user: "
            });
            line.push_str(&format!("{text:?}"));
        }
        line
    }
}

/// Lines describing every attachment for the system prompt (empty when there are none).
#[must_use]
pub fn describe_all(attachments: &[Attachment], lang: &str) -> String {
    attachments
        .iter()
        .map(|a| a.describe(lang))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Replaces quoted passages of a message with `@texteN` references, so the exact text never has
/// to be retyped by the model (measured: it corrupts texts ending with spaces) and stays local.
/// Quotes recognised: `'…'`, `"…"`, `« … »` and `“ … ”`. An apostrophe inside a word (l'image)
/// never opens a quote. `existing` is the number of text attachments already present.
#[must_use]
pub fn extract_quoted(message: &str, existing: usize) -> (String, Vec<String>) {
    let chars: Vec<char> = message.chars().collect();
    let mut out = String::new();
    let mut texts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let closer = match c {
            '«' => Some('»'),
            '“' => Some('”'),
            '"' => Some('"'),
            '\'' => Some('\''),
            _ => None,
        };
        // A quote opens at the start or after a space or punctuation, never inside a word.
        let opens = closer.is_some() && (i == 0 || !chars[i - 1].is_alphanumeric());
        if let (true, Some(close)) = (opens, closer) {
            // The closing mark must not be followed by a letter (that would be an apostrophe).
            let end = (i + 1..chars.len()).find(|&j| {
                chars[j] == close && chars.get(j + 1).is_none_or(|n| !n.is_alphanumeric())
            });
            if let Some(end) = end.filter(|&e| e > i + 1) {
                let mut inner = &chars[i + 1..end];
                // French typography pads « guillemets » and “quotes” with a (non-breaking) space
                // that is not part of the text; plain ASCII quotes keep their content exactly.
                if matches!(c, '«' | '“') {
                    let pad = |ch: &char| matches!(ch, ' ' | '\u{a0}' | '\u{202f}');
                    if inner.first().is_some_and(pad) {
                        inner = &inner[1..];
                    }
                    if inner.last().is_some_and(pad) {
                        inner = &inner[..inner.len() - 1];
                    }
                }
                texts.push(inner.iter().collect());
                out.push_str(&format!("@texte{}", existing + texts.len()));
                i = end + 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    (out, texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_passages_become_references_and_keep_their_exact_text() {
        let (message, texts) = extract_quoted("Nettoie ce texte : '  a   b '", 0);
        assert_eq!(message, "Nettoie ce texte : @texte1");
        assert_eq!(texts, ["  a   b "], "trailing spaces are kept exactly");
        let (message, texts) = extract_quoted("Compare « un » et \"deux\" puis “trois”", 2);
        assert_eq!(message, "Compare @texte3 et @texte4 puis @texte5");
        assert_eq!(texts, ["un", "deux", "trois"]);
    }

    #[test]
    fn apostrophes_inside_words_do_not_open_a_quote() {
        for text in [
            "Convertis l'image de l'utilisateur",
            "It's the user's file",
            "aujourd'hui j'ai deux fichiers",
        ] {
            assert_eq!(extract_quoted(text, 0), (text.to_owned(), vec![]), "{text}");
        }
        // A real quote after an apostrophe-bearing word still works.
        let (message, texts) = extract_quoted("Change l'ordre de 'b a c'", 0);
        assert_eq!(
            (message.as_str(), texts.as_slice()),
            ("Change l'ordre de @texte1", ["b a c".to_owned()].as_slice())
        );
    }

    #[test]
    fn unbalanced_or_empty_quotes_are_left_alone() {
        for text in ["il a dit 'bonjour", "vide '' ici", "un \" seul"] {
            assert_eq!(extract_quoted(text, 0).1.len(), 0, "{text}");
        }
    }

    #[test]
    fn the_model_gets_metadata_only_unless_the_user_shared_the_content() {
        let mut text = Attachment::text(1, "secret content");
        assert_eq!(
            text.describe("fr"),
            "@texte1 = texte1 (text/plain, 14 caractères)"
        );
        assert!(!text.describe("fr").contains("secret"));
        text.share_content = true;
        assert!(text.describe("fr").contains("\"secret content\""));
        let long = Attachment {
            share_content: true,
            ..Attachment::text(2, &"x".repeat(MAX_SHARED_CHARS + 1))
        };
        assert!(
            !long.describe("en").contains("xxxx"),
            "too long to share: metadata only"
        );
        let file = Attachment {
            n: 1,
            kind: Kind::File,
            name: "a.pdf".into(),
            mime: "application/pdf".into(),
            bytes: vec![0xFF, 0x00],
            share_content: true,
        };
        assert_eq!(
            file.describe("en"),
            "@fichier1 = a.pdf (application/pdf, 2 bytes)",
            "binary files never expose content"
        );
    }
}
