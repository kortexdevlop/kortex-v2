//! Shared Dioxus UI compiled for web and desktop from the same source.
use dioxus::prelude::*;
use kortex_shell::{self as shell, Frame};
use std::sync::OnceLock;

fn locale(lang: &str) -> &'static kortex_i18n::Catalog {
    static FR: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    static EN: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    if lang == "fr" {
        FR.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/fr.toml")).unwrap_or_default()
        })
    } else {
        EN.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/en.toml")).unwrap_or_default()
        })
    }
}

#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let l = locale(&(appearance.lang)());
    rsx! {
        Frame { title: l.t("ui.title"), subtitle: l.t("ui.subtitle"),
            p { class: "k-muted", {l.t("ui.empty")} }
        }
    }
}
