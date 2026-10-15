use dioxus::prelude::*;
use kortex_runtime::ValueMap;
use kortex_ui::{AutoTool, Button, Progress, RunFuture, Runner};

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
        rsx! { AutoTool { contract: p.0, lang: "fr".to_owned(), locale: kortex_i18n::Catalog::default(), runner: Runner(noop_runner) } }
    }
    let html = render(app, P(c));
    assert!(html.contains("Mon outil"), "{html}");
    assert!(html.contains("Taille"));
    assert!(
        !html.contains("Option pro"),
        "pro param must be hidden in Medium mode"
    );
}
