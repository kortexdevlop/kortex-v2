use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Type of an input, param or output port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortType {
    Text,
    RichText,
    Number,
    Integer,
    Bool,
    Enum,
    Color,
    Date,
    DateTime,
    Duration,
    Range,
    Json,
    Table,
    Secret,
    Url,
    /// `file<mime-pattern>`
    File(String),
    /// `files<mime-pattern>` (batch)
    Files(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortTypeError(pub String);

impl fmt::Display for PortTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "type de port inconnu : `{}`", self.0)
    }
}

impl FromStr for PortType {
    type Err = PortTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let simple = match s {
            "text" => Some(Self::Text),
            "rich_text" => Some(Self::RichText),
            "number" => Some(Self::Number),
            "integer" => Some(Self::Integer),
            "bool" => Some(Self::Bool),
            "enum" => Some(Self::Enum),
            "color" => Some(Self::Color),
            "date" => Some(Self::Date),
            "datetime" => Some(Self::DateTime),
            "duration" => Some(Self::Duration),
            "range" => Some(Self::Range),
            "json" => Some(Self::Json),
            "table" => Some(Self::Table),
            "secret" => Some(Self::Secret),
            "url" => Some(Self::Url),
            _ => None,
        };
        if let Some(t) = simple {
            return Ok(t);
        }
        let generic = |prefix: &str| {
            s.strip_prefix(prefix)
                .and_then(|r| r.strip_prefix('<'))
                .and_then(|r| r.strip_suffix('>'))
                .filter(|m| m.contains('/') && !m.is_empty())
                .map(str::to_owned)
        };
        if let Some(m) = generic("files") {
            return Ok(Self::Files(m));
        }
        if let Some(m) = generic("file") {
            return Ok(Self::File(m));
        }
        Err(PortTypeError(s.to_owned()))
    }
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Text => "text",
            Self::RichText => "rich_text",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Bool => "bool",
            Self::Enum => "enum",
            Self::Color => "color",
            Self::Date => "date",
            Self::DateTime => "datetime",
            Self::Duration => "duration",
            Self::Range => "range",
            Self::Json => "json",
            Self::Table => "table",
            Self::Secret => "secret",
            Self::Url => "url",
            Self::File(m) => return write!(f, "file<{m}>"),
            Self::Files(m) => return write!(f, "files<{m}>"),
        };
        f.write_str(s)
    }
}

impl<'de> Deserialize<'de> for PortType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for PortType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl PortType {
    /// Safe automatic conversion from `self` (an output) to `to` (an input).
    #[must_use]
    pub fn converts_to(&self, to: &Self) -> bool {
        use PortType::{File, Files, Table, Text};
        if self == to {
            return true;
        }
        match (self, to) {
            (File(a), File(b)) | (File(a), Files(b)) | (Files(a), Files(b)) => mime_matches(a, b),
            (Table, File(m)) => mime_matches("text/csv", m),
            (Text, File(m)) => mime_matches("text/plain", m),
            _ => false,
        }
    }
}

/// `image/png` matches `image/*`, `*/*` and itself.
fn mime_matches(have: &str, want: &str) -> bool {
    if want == "*/*" || have == want {
        return true;
    }
    match (have.split_once('/'), want.split_once('/')) {
        (Some((ha, _)), Some((wa, "*"))) => ha == wa || wa == "*",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_and_generic() {
        assert_eq!("text".parse(), Ok(PortType::Text));
        assert_eq!(
            "file<image/*>".parse(),
            Ok(PortType::File("image/*".into()))
        );
        assert_eq!(
            "files<application/pdf>".parse(),
            Ok(PortType::Files("application/pdf".into()))
        );
    }

    #[test]
    fn rejects_unknown() {
        assert!("bogus".parse::<PortType>().is_err());
        assert!("file<>".parse::<PortType>().is_err());
        assert!("file<nomime>".parse::<PortType>().is_err());
    }

    #[test]
    fn roundtrips_display() {
        for s in ["text", "secret", "file<image/*>", "files<text/csv>"] {
            assert_eq!(
                s.parse::<PortType>().map(|t| t.to_string()),
                Ok(s.to_owned())
            );
        }
    }

    #[test]
    fn conversions() {
        let png = PortType::File("image/png".into());
        assert!(png.converts_to(&PortType::File("image/*".into())));
        assert!(png.converts_to(&PortType::Files("image/*".into())));
        assert!(!png.converts_to(&PortType::File("application/pdf".into())));
        assert!(PortType::Table.converts_to(&PortType::File("text/csv".into())));
        assert!(!PortType::Secret.converts_to(&PortType::Text));
    }
}
