use dioxus::prelude::*;
use kortex_runtime::ValueMap;
use kortex_ui::{AutoTool, Button, DropZone, Progress, RunFuture, Runner, ToolHost, use_tool};

fn noop_runner(_: ValueMap, _: ValueMap) -> RunFuture {
    Box::pin(async { Ok(ValueMap::new()) })
}

fn render<P: Clone + 'static>(app: fn(P) -> Element, props: P) -> String {
    let mut vdom = VirtualDom::new_with_props(app, props);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn button_renders_with_design_class() {
    fn app(_: ()) -> Element {
        rsx! { Button { "Go" } }
    }
    let html = render(app, ());
    assert!(html.contains("k-btn"), "{html}");
    assert!(html.contains("Go"));
}

/// Desktop never relies on the WebView's file input: on macOS it cannot offer every file and fails
/// with "The open file operation failed" for most MIME types (Flow desktop bug, PLAN 15.11).
#[test]
fn desktop_drop_zone_opens_the_native_dialog_instead_of_a_file_input() {
    fn app(_: ()) -> Element {
        rsx! {
            DropZone {
                label: "Drop".to_owned(),
                hint: "or click".to_owned(),
                accept: Some("application/pdf".to_owned()),
                onfiles: |_| {},
            }
        }
    }
    let html = render(app, ());
    assert!(
        html.contains("<button") && html.contains("k-dropzone"),
        "{html}"
    );
    assert!(!html.contains("type=\"file\""), "{html}");
}

#[test]
fn progress_exposes_aria() {
    fn app(_: ()) -> Element {
        rsx! { Progress { value: 0.5 } }
    }
    let html = render(app, ());
    assert!(html.contains("aria-valuenow=\"50\""), "{html}");
}

#[test]
fn auto_tool_shows_medium_params_by_default() {
    let c = kortex_contract::parse(
        r#"
schema = 1
[tool]
id = "text.t"
version = "1.0.0"
universe = "text"
status = "draft"
ui = "auto"
platforms = ["web"]
name = { fr = "Mon outil", en = "My tool" }
tagline = { fr = "x", en = "x" }
slug = { fr = "t", en = "t" }
[[params]]
id = "size"
type = "integer"
default = 3
level = "medium"
label = { fr = "Taille", en = "Size" }
[[params]]
id = "secret_opt"
type = "bool"
level = "pro"
label = { fr = "Option pro", en = "Pro option" }
[[outputs]]
id = "o"
type = "text"
[permissions]
network = false
filesystem = "none"
clipboard = "none"
privacy = "local"
"#,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));

    #[derive(Clone, PartialEq)]
    struct P(kortex_contract::ToolContract);
    fn app(p: P) -> Element {
        rsx! { AutoTool { contract: p.0, lang: "fr".to_owned(), locale: kortex_i18n::Catalog::default(), runner: Runner(noop_runner), variant: None, host_mode: None, save_file: None } }
    }
    let html = render(app, P(c));
    assert!(html.contains("Mon outil"), "{html}");
    assert!(html.contains("Taille"));
    assert!(
        !html.contains("Option pro"),
        "pro param must be hidden in Medium mode"
    );
}

#[test]
fn custom_ui_reads_language_mode_and_texts_from_the_host() {
    let contract = kortex_contract::parse(
        r#"
schema = 1
[tool]
id = "text.t"
version = "1.0.0"
universe = "text"
status = "draft"
ui = "custom"
platforms = ["web"]
name = { fr = "Mon outil", en = "My tool" }
tagline = { fr = "x", en = "x" }
slug = { fr = "t", en = "t" }
[[params]]
id = "size"
type = "integer"
default = 3
level = "medium"
label = { fr = "Taille", en = "Size" }
[[params]]
id = "deep"
type = "bool"
level = "pro"
label = { fr = "Pro", en = "Pro" }
[[outputs]]
id = "o"
type = "text"
[permissions]
network = false
filesystem = "none"
clipboard = "none"
privacy = "local"
"#,
    )
    .unwrap_or_else(|e| unreachable!("{e}"));
    let locale = kortex_i18n::Catalog::from_toml("[ui]\ntitle = \"Titre perso\"\n")
        .unwrap_or_else(|e| unreachable!("{e}"));

    #[component]
    fn Ui() -> Element {
        let tool = use_tool();
        rsx! {
            p { id: "lang", "{tool.lang()}" }
            p { id: "mode", "{tool.mode().id()}" }
            p { id: "title", "{tool.t(\"ui.title\")}" }
            p { id: "missing", "{tool.t(\"ui.absent\")}" }
            p { id: "params", "{tool.visible_param_ids().join(\",\")}" }
            p { id: "running", "{tool.running()}" }
        }
    }

    #[derive(Clone, PartialEq)]
    struct P(kortex_contract::ToolContract, kortex_i18n::Catalog);
    fn app(p: P) -> Element {
        rsx! { ToolHost { contract: p.0, lang: "fr".to_owned(), locale: p.1, runner: Runner(noop_runner), variant: None, host_mode: Some("simple".to_owned()), save_file: None, Ui {} } }
    }
    let html = render(app, P(contract, locale));
    assert!(html.contains(">fr<"), "{html}");
    assert!(html.contains(">simple<"), "{html}");
    assert!(html.contains("Titre perso"), "{html}");
    assert!(
        html.contains("ui.absent"),
        "missing key falls back to the key: {html}"
    );
    assert!(
        html.contains("id=\"params\"></p>"),
        "simple mode shows no param: {html}"
    );
    assert!(html.contains(">false<"), "{html}");
}
