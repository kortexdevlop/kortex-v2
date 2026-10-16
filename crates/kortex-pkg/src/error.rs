use std::fmt;

/// Machine-readable failure class (tests and UIs branch on it, never on the message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    BadSignature,
    Integrity,
    Unsafe,
    Network,
    NotFound,
    Unsupported,
    Io,
    Parse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkgError {
    kind: Kind,
    message: String,
}

impl PkgError {
    pub fn new(kind: Kind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }
}

impl fmt::Display for PkgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PkgError {}

impl From<std::io::Error> for PkgError {
    fn from(e: std::io::Error) -> Self {
        Self::new(Kind::Io, e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PkgError>;
