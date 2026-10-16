use std::collections::BTreeMap;

use dioxus::prelude::*;
use kortex_pkg::ToolStatus;
use kortex_store::{Action, Catalogue, ToolRow, primary_action, universes, visible};

/// `names` = (fr, en); `state` = (installed version, available version, update available, supported).
fn st(
    id: &str,
    universe: &str,
    names: (&str, &str),
    state: (Option<&str>, &str, bool, bool),
) -> ToolStatus {
    ToolStatus {
        id: id.into(),
        name: BTreeMap::from([("fr".into(), names.0.into()), ("en".into(), names.1.into())]),
        universe: universe.into(),
        available: state.1.into(),
        installed: state.0.map(Into::into),
        update_available: state.2,
        supported: state.3,
    }
}

fn sample() -> Vec<ToolStatus> {
    vec![
        st(
            "pdf.merge",
            "pdf",
            ("Fusionner des PDF", "Merge PDF"),
            (None, "1.0.0", false, true),
        ),
        st(
            "secure.hash-calculator",
            "secure",
            ("Calculateur de hash", "Hash Calculator"),
            (Some("1.0.0"), "1.0.0", false, true),
        ),
        st(
            "image.convert",
            "image",
            ("Convertisseur d'images", "Image Converter"),
            (Some("1.0.0"), "1.1.0", true, true),
        ),
        st(
            "qr.generator",
            "qr",
            ("Générateur de QR code", "QR Code Generator"),
            (None, "1.0.0", false, false),
        ),
    ]
}

#[test]
fn search_ignores_case_and_accents_and_needs_every_word() {
    let all = sample();
    let ids = |q: &str, u: Option<&str>, lang: &str| {
        visible(&all, q, u, lang)
            .into_iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("generateur", None, "fr"), ["qr.generator"]);
    assert_eq!(ids("FUSIONNER pdf", None, "fr"), ["pdf.merge"]);
    assert_eq!(ids("merge", None, "en"), ["pdf.merge"]);
    assert_eq!(ids("hash", None, "en"), ["secure.hash-calculator"]);
    assert!(ids("zzz", None, "en").is_empty());
    assert_eq!(ids("", None, "en").len(), 4);
    assert_eq!(ids("", Some("pdf"), "en"), ["pdf.merge"]);
    assert!(ids("hash", Some("pdf"), "en").is_empty());
    assert_eq!(universes(&all), ["image", "pdf", "qr", "secure"]);
}

#[test]
fn the_primary_action_follows_the_install_state() {
    let all = sample();
    let by = |id: &str| all.iter().find(|s| s.id == id).expect("tool");
    assert_eq!(primary_action(by("pdf.merge")), Some(Action::Install));
    assert_eq!(
        primary_action(by("secure.hash-calculator")),
        Some(Action::Open)
    );
    assert_eq!(primary_action(by("image.convert")), Some(Action::Update));
    assert_eq!(
        primary_action(by("qr.generator")),
        None,
        "no build for this platform: nothing to offer"
    );
}

fn render(el: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(el);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn catalogue_renders_each_state_with_the_right_buttons() {
    fn app() -> Element {
        rsx! { Catalogue { items: sample(), lang: "fr".to_owned(), busy: vec!["pdf.merge".to_owned()], onaction: |_| {} } }
    }
    let html = render(app);
    assert!(
        html.contains("Fusionner des PDF") && html.contains("Convertisseur d&#39;images"),
        "{html}"
    );
    assert!(
        html.contains("En cours…"),
        "busy row shows progress: {html}"
    );
    assert!(html.contains("Mettre à jour") && html.contains("→ nouvelle version 1.1.0"));
    assert!(html.contains("Installé") && html.contains("Ouvrir"));
    assert!(html.contains("Indisponible sur cette plateforme"));
    assert_eq!(
        html.matches("Supprimer").count(),
        2,
        "only installed tools can be removed"
    );
    assert!(html.contains("disabled"), "busy buttons are disabled");
}

#[test]
fn english_labels_and_empty_state() {
    fn row() -> Element {
        rsx! { ToolRow { status: sample().remove(0), lang: "en".to_owned(), busy: false, onaction: |_| {} } }
    }
    let html = render(row);
    assert!(
        html.contains("Merge PDF") && html.contains(">Install<"),
        "{html}"
    );
    fn empty() -> Element {
        rsx! { Catalogue { items: vec![], lang: "fr".to_owned(), busy: vec![], onaction: |_| {} } }
    }
    assert!(render(empty).contains("Aucun outil ne correspond."));
}
