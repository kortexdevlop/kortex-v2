//! File typing by binary signature (MASTER_PROMPT §6.2): a file port is validated on what the
//! bytes are, never on the name or the MIME type a browser guessed from the extension.
//!
//! Pure and dependency-free so it runs identically in the browser, on desktop and headless.

/// A detected media type.
pub type Mime = &'static str;

fn starts(bytes: &[u8], offset: usize, magic: &[u8]) -> bool {
    bytes.get(offset..offset + magic.len()) == Some(magic)
}

/// Media type of `bytes` from well-known signatures, `None` when the content is not recognised.
/// UTF-8 text without NUL bytes is reported as `text/plain`.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Option<Mime> {
    if starts(bytes, 0, b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if starts(bytes, 0, &[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if starts(bytes, 0, b"GIF87a") || starts(bytes, 0, b"GIF89a") {
        Some("image/gif")
    } else if starts(bytes, 0, b"RIFF") && starts(bytes, 8, b"WEBP") {
        Some("image/webp")
    } else if starts(bytes, 0, b"BM") && bytes.len() >= 26 {
        Some("image/bmp")
    } else if starts(bytes, 0, b"II*\0") || starts(bytes, 0, b"MM\0*") {
        Some("image/tiff")
    } else if starts(bytes, 0, &[0, 0, 1, 0]) && bytes.len() >= 22 {
        Some("image/x-icon")
    } else if starts(bytes, 4, b"ftypavif") || starts(bytes, 4, b"ftypavis") {
        Some("image/avif")
    } else if starts(bytes, 4, b"ftypheic")
        || starts(bytes, 4, b"ftypheix")
        || starts(bytes, 4, b"ftypmif1")
    {
        Some("image/heic")
    } else if bytes
        .iter()
        .take(1024)
        .collect::<Vec<_>>()
        .windows(5)
        .any(|w| w.iter().map(|b| **b).eq(*b"%PDF-"))
    {
        Some("application/pdf")
    } else if starts(bytes, 0, b"PK\x03\x04") || starts(bytes, 0, b"PK\x05\x06") {
        Some("application/zip")
    } else if !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok() {
        Some("text/plain")
    } else {
        None
    }
}

/// Whether a media type satisfies a port pattern such as `image/*`, `application/pdf` or `*/*`.
#[must_use]
pub fn pattern_accepts(pattern: &str, mime: &str) -> bool {
    if pattern == "*/*" || pattern == mime {
        return true;
    }
    pattern
        .strip_suffix("/*")
        .is_some_and(|family| mime.split('/').next() == Some(family))
}

/// Patterns whose content can be verified by signature. Anything else (JSON, CSV, `text/*`
/// variants, …) is accepted as is: a signature check would only produce false rejections.
fn is_verifiable(pattern: &str) -> bool {
    pattern == "image/*"
        || pattern.starts_with("image/")
        || pattern == "application/pdf"
        || pattern == "application/zip"
}

/// A file whose bytes do not match the declared port pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeMismatch {
    pub expected: String,
    /// What the bytes actually look like (`None`: unrecognised content).
    pub found: Option<Mime>,
}

/// Checks `bytes` against a port pattern.
///
/// # Errors
/// [`TypeMismatch`] when the pattern is verifiable by signature and the content is of another
/// (or an unrecognised) type.
pub fn check(bytes: &[u8], pattern: &str) -> Result<(), TypeMismatch> {
    if !is_verifiable(pattern) {
        return Ok(());
    }
    let found = sniff(bytes);
    // `image/jpg`-style aliases are not used here: signatures report canonical types.
    match found {
        Some(mime) if pattern_accepts(pattern, mime) => Ok(()),
        _ => Err(TypeMismatch {
            expected: pattern.to_owned(),
            found,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const PDF: &[u8] = b"%PDF-1.7\n1 0 obj";

    #[test]
    fn recognises_common_signatures() {
        assert_eq!(sniff(PNG), Some("image/png"));
        assert_eq!(
            sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]),
            Some("image/jpeg")
        );
        assert_eq!(sniff(b"GIF89a\x01\0\x01\0"), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(
            sniff(&[
                b'B', b'M', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ]),
            Some("image/bmp")
        );
        assert_eq!(sniff(b"II*\0\x08\0\0\0"), Some("image/tiff"));
        assert_eq!(sniff(b"\0\0\0\x18ftypavif\0\0\0\0"), Some("image/avif"));
        assert_eq!(sniff(b"\0\0\0\x18ftypheic\0\0\0\0"), Some("image/heic"));
        assert_eq!(sniff(PDF), Some("application/pdf"));
        assert_eq!(sniff(b"PK\x03\x04\x14\0"), Some("application/zip"));
        assert_eq!(sniff("héllo".as_bytes()), Some("text/plain"));
        assert_eq!(sniff(&[0x01, 0x02, 0x00, 0xFF]), None);
        assert_eq!(sniff(b""), Some("text/plain"));
    }

    #[test]
    fn pdf_with_a_short_preamble_is_still_a_pdf() {
        let mut bytes = vec![b' '; 100];
        bytes.extend_from_slice(PDF);
        assert_eq!(sniff(&bytes), Some("application/pdf"));
    }

    #[test]
    fn patterns_match_exactly_by_family_or_everything() {
        assert!(pattern_accepts("image/*", "image/png"));
        assert!(pattern_accepts("image/png", "image/png"));
        assert!(pattern_accepts("*/*", "application/zip"));
        assert!(!pattern_accepts("image/*", "application/pdf"));
        assert!(!pattern_accepts("application/pdf", "image/png"));
    }

    #[test]
    fn a_renamed_file_is_rejected_by_content() {
        assert!(check(PNG, "image/*").is_ok());
        assert!(check(PDF, "application/pdf").is_ok());
        let fake_image = check(b"just some text pretending to be a png", "image/*");
        assert_eq!(
            fake_image,
            Err(TypeMismatch {
                expected: "image/*".into(),
                found: Some("text/plain")
            })
        );
        assert!(check(PNG, "application/pdf").is_err());
        assert_eq!(
            check(&[1, 0, 2], "image/*").err().map(|e| e.found),
            Some(None)
        );
    }

    #[test]
    fn unverifiable_patterns_accept_anything() {
        assert!(check(&[1, 0, 2], "*/*").is_ok());
        assert!(check(b"{}", "application/json").is_ok());
        assert!(check(b"a,b", "text/csv").is_ok());
    }
}
