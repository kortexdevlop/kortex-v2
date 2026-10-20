//! Ready-made workflows (`apps/flow/templates/*.json`), in French and English, with what each one
//! needs (an AI account, files, a secret) so the library can say so before it is opened.
use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::engine::{NodeKind, Workflow, auto_layout};

const SOURCES: [&str; 12] = [
    include_str!("../templates/clean-slug.json"),
    include_str!("../templates/snake-case.json"),
    include_str!("../templates/each-line-upper.json"),
    include_str!("../templates/sort-unique.json"),
    include_str!("../templates/count-words.json"),
    include_str!("../templates/hash-file.json"),
    include_str!("../templates/merge-pdf.json"),
    include_str!("../templates/summarize-ai.json"),
    include_str!("../templates/extract-table-ai.json"),
    include_str!("../templates/classify-route.json"),
    include_str!("../templates/translate-ai.json"),
    include_str!("../templates/summarize-telegram.json"),
];

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub description: BTreeMap<String, String>,
    /// `ai`, `files`, or `secret:NAME`.
    #[serde(default)]
    pub needs: Vec<String>,
    #[serde(default)]
    pub samples: BTreeMap<String, String>,
    pub workflow: Workflow,
}

/// Every shipped template (a file that does not parse is skipped here and caught by the tests).
pub fn all() -> &'static [Template] {
    static ALL: OnceLock<Vec<Template>> = OnceLock::new();
    ALL.get_or_init(|| {
        SOURCES
            .iter()
            .filter_map(|text| serde_json::from_str(text).ok())
            .collect()
    })
}

fn pick<'a>(map: &'a BTreeMap<String, String>, lang: &str) -> &'a str {
    map.get(lang)
        .or_else(|| map.get("en"))
        .map_or("", String::as_str)
}

impl Template {
    #[must_use]
    pub fn name(&self, lang: &str) -> &str {
        pick(&self.name, lang)
    }

    #[must_use]
    pub fn description(&self, lang: &str) -> &str {
        pick(&self.description, lang)
    }

    /// The workflow to open: the sample text in the user's language, named after the template,
    /// laid out.
    #[must_use]
    pub fn instantiate(&self, lang: &str) -> Workflow {
        let mut workflow = self.workflow.clone();
        workflow.name = self.name(lang).to_owned();
        let sample = pick(&self.samples, lang);
        for node in &mut workflow.nodes {
            if let NodeKind::Text { value } = &mut node.kind
                && value == "{sample}"
            {
                sample.clone_into(value);
            }
        }
        auto_layout(&mut workflow);
        workflow
    }
}
