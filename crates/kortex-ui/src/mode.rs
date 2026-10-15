use kortex_contract::{Param, ParamLevel, ToolContract};

/// Global / per-tool complexity mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Simple,
    #[default]
    Medium,
    Pro,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Simple, Self::Medium, Self::Pro];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Pro => "pro",
        }
    }

    #[must_use]
    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == s)
    }

    /// Highest param level shown in this mode. `Simple` shows no param (smart defaults).
    const fn max_level(self) -> Option<ParamLevel> {
        match self {
            Self::Simple => None,
            Self::Medium => Some(ParamLevel::Medium),
            Self::Pro => Some(ParamLevel::Pro),
        }
    }
}

/// Params to display in `mode`: Simple = none, Medium = simple+medium, Pro = all.
#[must_use]
pub fn visible_params(c: &ToolContract, mode: Mode) -> Vec<&Param> {
    let Some(max) = mode.max_level() else {
        return Vec::new();
    };
    c.params
        .iter()
        .filter(|p| p.level.unwrap_or(ParamLevel::Medium) <= max)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> ToolContract {
        let text = r#"
schema = 1
[tool]
id = "text.t"
version = "1.0.0"
universe = "text"
status = "draft"
ui = "auto"
platforms = ["web"]
name = { fr = "T", en = "T" }
tagline = { fr = "x", en = "x" }
slug = { fr = "t", en = "t" }
[[params]]
id = "a"
type = "integer"
default = 1
level = "simple"
[[params]]
id = "b"
type = "bool"
level = "medium"
[[params]]
id = "c"
type = "bool"
level = "pro"
[[outputs]]
id = "o"
type = "text"
[permissions]
network = false
filesystem = "none"
clipboard = "none"
privacy = "local"
"#;
        kortex_contract::parse(text).unwrap_or_else(|e| unreachable!("{e}"))
    }

    #[test]
    fn modes_filter_params() {
        let c = contract();
        let ids = |m| {
            visible_params(&c, m)
                .iter()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>()
        };
        assert!(ids(Mode::Simple).is_empty());
        assert_eq!(ids(Mode::Medium), ["a", "b"]);
        assert_eq!(ids(Mode::Pro), ["a", "b", "c"]);
    }

    #[test]
    fn mode_ids_roundtrip() {
        for m in Mode::ALL {
            assert_eq!(Mode::from_id(m.id()), Some(m));
        }
    }
}
