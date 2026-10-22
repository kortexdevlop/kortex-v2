//! What the player needs to know about a tool, read from its payload files.
use kortex_contract::{ToolContract, UiKind};

/// Contract, locales and glyph of a tool (the engine travels separately).
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// `tool.toml` as shipped, served as is by `--engine`'s `contract` method.
    pub contract_text: String,
    pub contract: ToolContract,
    pub locale_fr: String,
    pub locale_en: String,
    pub glyph: String,
}

/// A tool and its `engine.wasm`.
pub struct Package {
    pub tool: Tool,
    pub engine: Vec<u8>,
}

impl Tool {
    /// # Errors
    /// The contract does not parse, or the tool needs its own interface (not supported: RULES 3.8).
    pub fn new(
        contract_text: String,
        locale_fr: String,
        locale_en: String,
        glyph: String,
    ) -> Result<Self, String> {
        let contract = kortex_contract::parse(&contract_text)
            .map_err(|e| format!("tool.toml invalide : {e}"))?;
        if contract.tool.ui == UiKind::Custom {
            return Err(format!(
                "{} a une interface personnalisée : le lecteur ne sait afficher que les interfaces générées",
                contract.tool.id
            ));
        }
        Ok(Self {
            contract_text,
            contract,
            locale_fr,
            locale_en,
            glyph,
        })
    }

    /// Run delay in milliseconds (the contract's `timeout_s`, 120 s by default).
    #[must_use]
    pub fn timeout_ms(&self) -> u32 {
        self.contract
            .limits
            .timeout_s
            .unwrap_or(120)
            .saturating_mul(1000)
    }

    /// Window title: "Kortex — <English name>".
    #[must_use]
    pub fn title(&self) -> String {
        let name = self
            .contract
            .tool
            .name
            .get("en")
            .map_or(self.contract.tool.id.as_str(), String::as_str);
        format!("Kortex — {name}")
    }

    /// Locale source of `lang` (`fr` or anything else for English).
    #[must_use]
    pub fn locale_source(&self, lang: &str) -> &str {
        if lang == "fr" {
            &self.locale_fr
        } else {
            &self.locale_en
        }
    }
}

impl Package {
    /// Reads the files of a payload.
    ///
    /// # Errors
    /// A required file is missing, or the contract is unusable.
    pub fn from_files(files: &[(String, Vec<u8>)]) -> Result<Self, String> {
        let get = |name: &str| {
            files
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, data)| data.as_slice())
                .ok_or_else(|| format!("fichier absent du paquet : {name}"))
        };
        let text = |name: &str| {
            String::from_utf8(get(name)?.to_vec())
                .map_err(|_| format!("{name} n'est pas de l'UTF-8"))
        };
        Ok(Self {
            tool: Tool::new(
                text("tool.toml")?,
                text("locales/fr.toml")?,
                text("locales/en.toml")?,
                text("glyph.svg").unwrap_or_default(),
            )?,
            engine: get("engine.wasm")?.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRACT: &str = include_str!("../../../tools/text/case-converter/tool.toml");
    const FR: &str = include_str!("../../../tools/text/case-converter/locales/fr.toml");
    const EN: &str = include_str!("../../../tools/text/case-converter/locales/en.toml");

    fn files() -> Vec<(String, Vec<u8>)> {
        vec![
            ("tool.toml".to_owned(), CONTRACT.as_bytes().to_vec()),
            ("locales/fr.toml".to_owned(), FR.as_bytes().to_vec()),
            ("locales/en.toml".to_owned(), EN.as_bytes().to_vec()),
            ("glyph.svg".to_owned(), b"<svg/>".to_vec()),
            ("engine.wasm".to_owned(), vec![0, 97, 115, 109]),
        ]
    }

    #[test]
    fn a_package_reads_its_files() {
        let package = Package::from_files(&files());
        let tool = package.as_ref().map(|p| &p.tool);
        assert_eq!(
            tool.map(|t| t.contract.tool.id.as_str()),
            Ok("text.case-converter")
        );
        assert_eq!(
            tool.map(Tool::title),
            Ok("Kortex — Case Converter".to_owned())
        );
        assert_eq!(tool.map(Tool::timeout_ms), Ok(10_000));
        assert_eq!(package.map(|p| p.engine), Ok(vec![0, 97, 115, 109]));
    }

    #[test]
    fn locale_follows_the_language() {
        let tool = Package::from_files(&files()).map(|p| p.tool);
        assert_eq!(tool.as_ref().map(|t| t.locale_source("fr")), Ok(FR));
        assert_eq!(tool.as_ref().map(|t| t.locale_source("en")), Ok(EN));
        assert_eq!(tool.as_ref().map(|t| t.locale_source("de")), Ok(EN));
    }

    #[test]
    fn a_missing_file_or_a_broken_contract_is_reported() {
        let mut missing = files();
        missing.retain(|(n, _)| n != "engine.wasm");
        assert!(Package::from_files(&missing).is_err_and(|e| e.contains("engine.wasm")));
        let mut broken = files();
        broken[0].1 = b"[tool".to_vec();
        assert!(Package::from_files(&broken).is_err());
    }
}
