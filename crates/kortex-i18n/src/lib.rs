//! Locale catalogs from `locales/<lang>.toml` (`[section] key = "text"` -> `section.key`).
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog(BTreeMap<String, String>);

impl Catalog {
    /// # Errors
    /// Invalid TOML or non-string leaf values.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let mut map = BTreeMap::new();
        for (section, value) in &table {
            let toml::Value::Table(inner) = value else {
                return Err(format!("`{section}` doit être une table [section]"));
            };
            for (key, v) in inner {
                let toml::Value::String(s) = v else {
                    return Err(format!("`{section}.{key}` doit être une chaîne"));
                };
                map.insert(format!("{section}.{key}"), s.clone());
            }
        }
        Ok(Self(map))
    }

    /// Translation for `key`, or the key itself when missing (visible, never a panic).
    #[must_use]
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map_or(key, String::as_str)
    }

    /// Translation with `{name}` placeholders replaced.
    #[must_use]
    pub fn fmt(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.t(key).to_owned();
        for (name, value) in args {
            s = s.replace(&format!("{{{name}}}"), value);
        }
        s
    }

    /// Keys present here but absent from `other`.
    #[must_use]
    pub fn missing_in(&self, other: &Self) -> Vec<String> {
        self.0
            .keys()
            .filter(|k| !other.0.contains_key(*k))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_and_format() {
        let c = Catalog::from_toml("[ui]\ngreet = \"Salut {name}\"\n").unwrap_or_default();
        assert_eq!(c.t("ui.greet"), "Salut {name}");
        assert_eq!(c.fmt("ui.greet", &[("name", "Léa")]), "Salut Léa");
        assert_eq!(c.t("ui.nope"), "ui.nope");
    }

    #[test]
    fn detects_missing_keys() {
        let fr = Catalog::from_toml("[a]\nx=\"1\"\ny=\"2\"\n").unwrap_or_default();
        let en = Catalog::from_toml("[a]\nx=\"1\"\n").unwrap_or_default();
        assert_eq!(fr.missing_in(&en), vec!["a.y".to_owned()]);
        assert!(en.missing_in(&fr).is_empty());
    }

    #[test]
    fn rejects_bad_shape() {
        assert!(Catalog::from_toml("x = \"1\"").is_err());
    }
}
