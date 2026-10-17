//! Pure {{Name}} rules, independent of Dioxus and platform access.

/// Normalizes a user query: trimmed, lower-case, single-spaced.
#[must_use]
pub fn normalize(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
