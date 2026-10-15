//! Pure engine: no UI, no direct platform access.

/// Engine error codes (translated in `locales/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolError {
    EmptyInput,
}

/// Placeholder behaviour: returns the input unchanged.
///
/// # Errors
/// `ToolError::EmptyInput` when `text` is empty.
pub fn run(text: &str) -> Result<String, ToolError> {
    if text.is_empty() {
        return Err(ToolError::EmptyInput);
    }
    Ok(text.to_owned())
}
