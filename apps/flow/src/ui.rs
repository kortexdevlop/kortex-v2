//! Kortex Flow UI: a node canvas shared by web and desktop. Tools are placed as cards, chained by
//! dragging from an output handle to an input handle (or by adding a tool after the selected
//! one), and run locally node by node. All editing rules live in [`crate::canvas`].
use std::collections::BTreeMap;
use std::sync::OnceLock;

use dioxus::prelude::*;
use kortex_contract::{ParamLevel, PortType};
use kortex_runtime::{CancelToken, Value, ValueMap};
use kortex_shell::{self as shell, Frame};

use crate::canvas::{
    self, History, NODE_W, PortView, RunState, States, View, can_connect, connect, fit, node_ports,
    port_point,
};
use crate::catalogue::{ports, usable_tools};
use crate::engine::{
    Endpoint, Execution, Node, NodeKind, Step, Workflow, auto_layout, from_json, to_json, validate,
};

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

fn summary(value: &Value) -> String {
    match value {
        Value::Text(t) => t.chars().take(2000).collect(),
        Value::Number(n) => n.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::File(f) => format!("{} ({} B)", f.name, f.bytes.len()),
        Value::Files(files) => format!("{} files", files.len()),
        Value::Table(t) => format!("{} × {}", t.rows.len(), t.columns.len()),
    }
}

fn node_title(node: &Node, l: &kortex_i18n::Catalog, lang: &str) -> String {
    match &node.kind {
        NodeKind::Text { .. } => l.t("ui.text_node").to_owned(),
        NodeKind::Tool { tool, .. } => {
            shell::find(tool).map_or_else(|| tool.clone(), |e| shell::name(e, lang))
        }
    }
}

/// What the pointer is doing on the canvas.
#[derive(Debug, Clone, PartialEq)]
enum Drag {
    Idle,
    Pan {
        x: f64,
        y: f64,
    },
    Node {
        id: String,
        x: f64,
        y: f64,
        px: f64,
        py: f64,
        moved: bool,
    },
    Link {
        from: Endpoint,
    },
}

/// Canvas element geometry on the screen (left, top, width, height).
type Frame4 = (f64, f64, f64, f64);

fn ui_error(l: &kortex_i18n::Catalog, code: &str, detail: Option<&str>) -> String {
    let key = format!("errors.{code}");
    let text = l.t(&key);
    let base = if text == key {
        code.to_owned()
    } else {
        text.to_owned()
    };
    detail.map_or(base.clone(), |d| format!("{base} ({d})"))
}

/// Runs the workflow node by node, reporting live state; stops at the first failure.
async fn execute(
    workflow: Workflow,
    token: CancelToken,
    mut states: Signal<States>,
    mut results: Signal<BTreeMap<String, ValueMap>>,
    mut log: Signal<Vec<String>>,
    lang: String,
) -> bool {
    let l = locale(&lang);
    let mut execution = match Execution::new(&workflow, ports()) {
        Ok(execution) => execution,
        Err(failure) => {
            log.write().push(format!(
                "✗ {}",
                ui_error(l, &failure.code, failure.detail.as_deref())
            ));
            return false;
        }
    };
    let mut ok = true;
    loop {
        match execution.next(&token) {
            Step::Run(job) => {
                states.write().insert(job.node.clone(), RunState::Running);
                let outcome = crate::runner::run(&job).await;
                let node = job.node.clone();
                match &outcome {
                    Ok(outputs) => {
                        states.write().insert(node.clone(), RunState::Done);
                        results.write().insert(node, outputs.clone());
                    }
                    Err(_) => {
                        states.write().insert(node, RunState::Failed);
                    }
                }
                execution.finish(outcome);
            }
            Step::Done(report) => {
                for (id, outputs) in report.results {
                    states.write().insert(id.clone(), RunState::Done);
                    results.write().insert(id, outputs);
                }
                log.write().extend(report.journal);
                break;
            }
            Step::Failed(failure) => {
                ok = false;
                if let Some(node) = &failure.node {
                    states.write().insert(node.clone(), RunState::Failed);
                }
                for (id, outputs) in failure.report.results {
                    results.write().insert(id, outputs);
                }
                log.write().extend(failure.report.journal);
                log.write().push(format!(
                    "✗ {} — {}",
                    failure.node.as_deref().unwrap_or("-"),
                    ui_error(l, &failure.code, failure.detail.as_deref())
                ));
                break;
            }
        }
    }
    ok
}

async fn read_frame(mount: Option<std::rc::Rc<MountedData>>) -> Option<Frame4> {
    let rect = mount?.get_client_rect().await.ok()?;
    Some((
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ))
}

#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let pro = (appearance.mode)() == "pro";
    let (loaded, storage_error) = use_hook(|| shell::load::<Workflow>("flow", "workflow"));
    let mut workflow = use_signal(move || {
        let mut w = loaded.clone();
        auto_layout(&mut w);
        w
    });
    let mut history = use_signal(History::default);
    let mut view = use_signal(View::default);
    let mut drag = use_signal(|| Drag::Idle);
    let mut cursor = use_signal(|| (0.0_f64, 0.0_f64));
    let mut selected = use_signal(|| None::<String>);
    let mut frame = use_signal(|| (0.0_f64, 0.0_f64, 800.0_f64, 560.0_f64));
    let mut mount = use_signal(|| None::<std::rc::Rc<MountedData>>);
    let mut import_text = use_signal(String::new);
    let mut note = use_signal(String::new);
    let mut search = use_signal(String::new);
    let mut states = use_signal(States::new);
    let results = use_signal(BTreeMap::<String, ValueMap>::new);
    let log = use_signal(Vec::<String>::new);
    let mut outcome = use_signal(|| None::<bool>);
    let mut running = use_signal(|| false);
    let mut cancel = use_signal(CancelToken::default);
    use_effect(move || {
        shell::save("flow", "workflow", &*workflow.read());
    });

    let current = workflow();
    let catalogue = ports();
    let issues = validate(&current, catalogue);
    let tools = usable_tools(&lang);
    let issue_text = |code: &str, detail: &str| {
        let key = format!("issues.{code}");
        let text = l.t(&key);
        if detail.is_empty() {
            text.to_owned()
        } else {
            format!("{text} ({detail})")
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let missing: Vec<String> = current
        .nodes
        .iter()
        .filter_map(|n| match &n.kind {
            NodeKind::Tool { tool, .. } if shell::installed_binary(tool).is_none() => {
                Some(tool.clone())
            }
            _ => None,
        })
        .collect();
    #[cfg(target_arch = "wasm32")]
    let missing: Vec<String> = Vec::new();
    let can_run =
        issues.is_empty() && missing.is_empty() && !running() && !current.nodes.is_empty();
    let v = view();
    let live = drag();

    // Applies an edit as one undo step.
    let mut edit = move |change: &dyn Fn(&mut Workflow)| {
        let before = workflow.peek().clone();
        history.write().record(&before);
        change(&mut workflow.write());
    };
    let show_all = move || {
        let m = mount.peek().clone();
        spawn(async move {
            if let Some(f) = read_frame(m).await {
                frame.set(f);
            }
            let (_, _, w, h) = frame();
            view.set(fit(&workflow.peek(), ports(), w, h));
        });
    };
    let mut add = move |kind: NodeKind| {
        let before = workflow.peek().clone();
        history.write().record(&before);
        let after = selected.peek().clone();
        let id = canvas::add_node(&mut workflow.write(), ports(), kind, after.as_deref(), None);
        selected.set(Some(id.clone()));
        states.write().clear();
        // Keep the new card in sight: pan (never zoom out) when it landed outside the canvas.
        let m = mount.peek().clone();
        spawn(async move {
            if let Some(f) = read_frame(m).await {
                frame.set(f);
            }
            let (_, _, w, h) = frame();
            let v = view.peek().to_owned();
            let spot = workflow
                .peek()
                .nodes
                .iter()
                .find(|n| n.id == id)
                .and_then(|n| n.pos);
            if let Some(p) = spot {
                let (x, y) = (f64::from(p.x) * v.zoom + v.x, f64::from(p.y) * v.zoom + v.y);
                if x < 0.0 || y < 0.0 || x + NODE_W * v.zoom > w || y + 130.0 * v.zoom > h {
                    view.set(View {
                        x: w / 2.0 - (f64::from(p.x) + NODE_W / 2.0) * v.zoom,
                        y: h / 2.0 - (f64::from(p.y) + 60.0) * v.zoom,
                        zoom: v.zoom,
                    });
                }
            }
        });
    };
    let mut undo = move || {
        let now = workflow.peek().clone();
        let previous = history.write().undo(&now);
        if let Some(previous) = previous {
            workflow.set(previous);
        }
    };
    let mut redo = move || {
        let now = workflow.peek().clone();
        let next = history.write().redo(&now);
        if let Some(next) = next {
            workflow.set(next);
        }
    };
    let selected_now = selected();
    let run_lang = lang.clone();

    rsx! {
        Frame { title: l.t("ui.title"), subtitle: l.t("ui.subtitle"),
            style { {include_str!("flow.css")} }
            if storage_error {
                p { class: "app-note", role: "alert", {l.t("ui.storage_error")} }
            }
            div { class: "flow-bar", role: "toolbar", "aria-label": l.t("ui.toolbar"),
                label { class: "k-field flow-name",
                    span { class: "k-muted", {l.t("ui.name")} }
                    input {
                        class: "k-input",
                        value: "{current.name}",
                        onfocus: move |_| {
                            let before = workflow.peek().clone();
                            history.write().record(&before);
                        },
                        oninput: move |e| workflow.write().name = e.value(),
                    }
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    title: l.t("ui.undo"),
                    "aria-label": l.t("ui.undo"),
                    disabled: !history.read().can_undo(),
                    onclick: move |_| undo(),
                    "↶"
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    title: l.t("ui.redo"),
                    "aria-label": l.t("ui.redo"),
                    disabled: !history.read().can_redo(),
                    onclick: move |_| redo(),
                    "↷"
                }
                span { class: "flow-sep" }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    "aria-label": l.t("ui.zoom_out"),
                    onclick: move |_| {
                        let (_, _, w, h) = frame();
                        view.set(view().zoomed(w / 2.0, h / 2.0, 1.0 / 1.25));
                    },
                    "−"
                }
                span { class: "flow-zoom", "aria-live": "polite", "{(v.zoom * 100.0).round()}%" }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    "aria-label": l.t("ui.zoom_in"),
                    onclick: move |_| {
                        let (_, _, w, h) = frame();
                        view.set(view().zoomed(w / 2.0, h / 2.0, 1.25));
                    },
                    "+"
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    onclick: move |_| show_all(),
                    {l.t("ui.fit")}
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    disabled: current.nodes.is_empty(),
                    onclick: move |_| {
                        edit(&|w| {
                            for node in &mut w.nodes {
                                node.pos = None;
                            }
                            auto_layout(w);
                        });
                        show_all();
                    },
                    {l.t("ui.arrange")}
                }
                span { class: "flow-sep" }
                button {
                    class: "k-btn",
                    r#type: "button",
                    disabled: !can_run,
                    onclick: move |_| {
                        states.set(current_waiting(&workflow.peek()));
                        let mut results = results;
                        let mut log = log;
                        results.write().clear();
                        log.write().clear();
                        outcome.set(None);
                        running.set(true);
                        let token = CancelToken::default();
                        cancel.set(token.clone());
                        let wf = workflow.peek().clone();
                        let lang = run_lang.clone();
                        spawn(async move {
                            let ok = execute(wf, token, states, results, log, lang).await;
                            outcome.set(Some(ok));
                            running.set(false);
                        });
                    },
                    "▶ " {l.t("ui.run")}
                }
                if running() {
                    button {
                        class: "k-btn",
                        "data-variant": "ghost",
                        r#type: "button",
                        onclick: move |_| {
                            cancel.peek().cancel();
                            crate::runner::cancel();
                        },
                        {l.t("ui.cancel")}
                    }
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    onclick: move |_| {
                        let bytes = to_json(&workflow.peek()).into_bytes();
                        save_file("workflow.json", "application/json", &bytes, note, l);
                    },
                    {l.t("ui.export")}
                }
            }
            div { class: "flow-stage",
                // Palette: click adds the tool after the selected node and links them.
                aside { class: "flow-palette", "aria-label": l.t("ui.palette"),
                    h2 { {l.t("ui.palette")} }
                    input {
                        class: "k-input",
                        r#type: "search",
                        placeholder: l.t("ui.search"),
                        "aria-label": l.t("ui.search"),
                        value: "{search}",
                        oninput: move |e| search.set(e.value()),
                    }
                    p { class: "flow-hint", {l.t("ui.palette_hint")} }
                    button {
                        class: "flow-pick",
                        "data-add": "text",
                        r#type: "button",
                        onclick: move |_| add(NodeKind::Text { value: String::new() }),
                        {l.t("ui.text_node")}
                    }
                    {palette(&tools, &search(), add)}
                }
                div {
                    class: "flow-canvas",
                    id: "flow-canvas",
                    tabindex: "0",
                    role: "application",
                    "aria-label": l.t("ui.canvas"),
                    "data-drag": if matches!(live, Drag::Pan { .. }) { "pan" } else { "" },
                    onmounted: move |e| {
                        mount.set(Some(e.data()));
                        let data = e.data();
                        spawn(async move {
                            if let Some(f) = read_frame(Some(data)).await {
                                frame.set(f);
                                if !workflow.peek().nodes.is_empty() {
                                    view.set(fit(&workflow.peek(), ports(), f.2, f.3));
                                }
                            }
                        });
                    },
                    onpointerdown: move |e| {
                        let p = e.client_coordinates();
                        drag.set(Drag::Pan { x: p.x, y: p.y });
                        selected.set(None);
                        let m = mount.peek().clone();
                        spawn(async move {
                            if let Some(f) = read_frame(m).await {
                                frame.set(f);
                            }
                        });
                    },
                    onpointermove: move |e| {
                        let p = e.client_coordinates();
                        let (ox, oy, _, _) = frame();
                        cursor.set(view().to_canvas(p.x - ox, p.y - oy));
                        match drag() {
                            Drag::Pan { x, y } => {
                                view.set(view().panned(p.x - x, p.y - y));
                                drag.set(Drag::Pan { x: p.x, y: p.y });
                            }
                            Drag::Node { id, x, y, px, py, moved } => {
                                let zoom = view().zoom;
                                let (nx, ny) = (px + (p.x - x) / zoom, py + (p.y - y) / zoom);
                                if !moved {
                                    let before = workflow.peek().clone();
                                    history.write().record(&before);
                                }
                                canvas::move_node(&mut workflow.write(), &id, nx.round() as i32, ny.round() as i32);
                                drag.set(Drag::Node { id, x: p.x, y: p.y, px: nx, py: ny, moved: true });
                            }
                            _ => {}
                        }
                    },
                    onpointerup: move |_| drag.set(Drag::Idle),
                    onpointerleave: move |_| {
                        if matches!(drag(), Drag::Pan { .. } | Drag::Node { .. }) {
                            drag.set(Drag::Idle);
                        }
                    },
                    onwheel: move |e| {
                        e.prevent_default();
                        let dy = e.delta().strip_units().y;
                        let (ox, oy, _, _) = frame();
                        let p = e.client_coordinates();
                        let factor = if dy < 0.0 { 1.1 } else { 1.0 / 1.1 };
                        view.set(view().zoomed(p.x - ox, p.y - oy, factor));
                    },
                    onkeydown: move |e| {
                        let ctrl = e.modifiers().ctrl() || e.modifiers().meta();
                        match e.key() {
                            Key::Delete | Key::Backspace => {
                                let target = selected.peek().clone();
                                if let Some(id) = target {
                                    edit(&|w| canvas::remove_node(w, &id));
                                    selected.set(None);
                                    states.write().clear();
                                }
                            }
                            Key::Escape => {
                                selected.set(None);
                                drag.set(Drag::Idle);
                            }
                            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("z") => {
                                e.prevent_default();
                                if e.modifiers().shift() { redo() } else { undo() }
                            }
                            Key::Character(c) if ctrl && c.eq_ignore_ascii_case("y") => {
                                e.prevent_default();
                                redo();
                            }
                            _ => {}
                        }
                    },
                    if current.nodes.is_empty() {
                        div { class: "flow-empty", role: "status",
                            strong { {l.t("ui.empty_title")} }
                            span { {l.t("ui.empty")} }
                        }
                    }
                    div {
                        class: "flow-layer",
                        style: "transform: translate({v.x}px, {v.y}px) scale({v.zoom});",
                        svg { class: "flow-links",
                            for edge in current.edges.clone() {
                                {link(&edge, &current, workflow, history)}
                            }
                            if let Drag::Link { from } = &live {
                                if let Some(a) = current.nodes.iter().find(|n| n.id == from.node).and_then(|n| port_point(n, catalogue, true, &from.port)) {
                                    path { class: "flow-link-live", d: canvas::link_path(a, cursor()) }
                                }
                            }
                        }
                        for node in current.nodes.clone() {
                            {
                                card(
                                    &node,
                                    &current,
                                    &live,
                                    selected_now.as_deref() == Some(node.id.as_str()),
                                    states().get(&node.id).copied(),
                                    l,
                                    &lang,
                                    Handlers { workflow, history, drag, selected, mount, frame },
                                )
                            }
                        }
                    }
                    {minimap(&current, v, frame())}
                }
                aside { class: "flow-inspector", "aria-label": l.t("ui.inspector"),
                    h2 { {l.t("ui.inspector")} }
                    if let Some(node) = selected_now.as_deref().and_then(|id| current.nodes.iter().find(|n| n.id == id)).cloned() {
                        {inspector(&node, &current, workflow, history, selected, results, pro, l, &lang, note)}
                    } else {
                        p { class: "flow-hint", {l.t("ui.inspector_empty")} }
                        p { class: "flow-hint", {l.t("ui.shortcuts")} }
                    }
                }
            }
            if !issues.is_empty() {
                div { class: "app-note", role: "alert",
                    strong { {l.t("ui.issues")} }
                    ul {
                        for (i , item) in issues.iter().enumerate() {
                            li { key: "{i}",
                                if let Some(node) = &item.node {
                                    "{node} — "
                                }
                                {issue_text(item.code, &item.detail)}
                            }
                        }
                    }
                }
            }
            if !missing.is_empty() {
                p { class: "app-note", role: "status", "{l.t(\"ui.not_installed\")} {missing.join(\", \")}" }
            }
            if !note().is_empty() {
                p { class: "app-note", role: "status", "{note}" }
            }
            if let Some(ok) = outcome() {
                div { class: "app-note", role: if ok { "status" } else { "alert" },
                    strong { if ok { {l.t("ui.run_ok")} } else { {l.t("ui.run_failed")} } }
                    pre { class: "flow-log", "{log.read().join(\"\\n\")}" }
                }
            }
            details { class: "flow-import",
                summary { {l.t("ui.import")} }
                textarea {
                    class: "k-input k-textarea",
                    rows: "6",
                    spellcheck: "false",
                    value: "{import_text}",
                    oninput: move |e| import_text.set(e.value()),
                }
                button {
                    class: "k-btn",
                    r#type: "button",
                    disabled: import_text().trim().is_empty(),
                    onclick: move |_| match from_json(&import_text()) {
                        Ok(imported) => {
                            let before = workflow.peek().clone();
                            history.write().record(&before);
                            let mut imported = imported;
                            auto_layout(&mut imported);
                            workflow.set(imported);
                            import_text.set(String::new());
                            selected.set(None);
                            states.write().clear();
                            outcome.set(None);
                            note.set(String::new());
                            show_all();
                        }
                        Err(issue) => note.set(issue_text(issue.code, &issue.detail)),
                    },
                    {l.t("ui.import_apply")}
                }
            }
        }
    }
}

fn current_waiting(workflow: &Workflow) -> States {
    workflow
        .nodes
        .iter()
        .map(|n| (n.id.clone(), RunState::Waiting))
        .collect()
}

fn save_file(
    name: &str,
    mime: &str,
    bytes: &[u8],
    mut note: Signal<String>,
    l: &kortex_i18n::Catalog,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = l;
        kortex_platform::document::download(name, mime, bytes);
        note.set(String::new());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mime;
        note.set(match kortex_platform::native::save_download(name, bytes) {
            Ok(path) => format!("{} {path}", l.t("ui.saved")),
            Err(error) => error,
        });
    }
}

/// Tool buttons grouped by universe (the id prefix), filtered by the search text.
fn palette(
    tools: &[(String, String)],
    query: &str,
    mut add: impl FnMut(NodeKind) + Copy + 'static,
) -> Element {
    let needle = query.trim().to_lowercase();
    let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (id, name) in tools {
        if needle.is_empty() || name.to_lowercase().contains(&needle) || id.contains(&needle) {
            let universe = id.split('.').next().unwrap_or("").to_owned();
            groups
                .entry(universe)
                .or_default()
                .push((id.clone(), name.clone()));
        }
    }
    rsx! {
        for (universe , items) in groups {
            div { key: "{universe}", class: "flow-group",
                span { class: "flow-group-name", "{universe}" }
                for (id , name) in items {
                    button {
                        key: "{id}",
                        class: "flow-pick",
                        "data-tool": "{id}",
                        r#type: "button",
                        onclick: move |_| {
                            add(NodeKind::Tool { tool: id.clone(), params: BTreeMap::new() });
                        },
                        "{name}"
                    }
                }
            }
        }
    }
}

fn link(
    edge: &crate::engine::Edge,
    current: &Workflow,
    mut workflow: Signal<Workflow>,
    mut history: Signal<History>,
) -> Element {
    let find = |id: &str| current.nodes.iter().find(|n| n.id == id);
    let (Some(a), Some(b)) = (find(&edge.from.node), find(&edge.to.node)) else {
        return rsx! {};
    };
    let (Some(from), Some(to)) = (
        port_point(a, ports(), true, &edge.from.port),
        port_point(b, ports(), false, &edge.to.port),
    ) else {
        return rsx! {};
    };
    let key = format!(
        "{}.{}>{}.{}",
        edge.from.node, edge.from.port, edge.to.node, edge.to.port
    );
    let (mx, my) = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    let target = edge.to.clone();
    let d = canvas::link_path(from, to);
    rsx! {
        g { key: "{key}", class: "flow-link", "data-edge": "{key}",
            path { class: "hit", d: "{d}" }
            path { d: "{d}" }
            circle {
                class: "cut",
                cx: "{mx}",
                cy: "{my}",
                r: "10",
                "data-cut": "{key}",
                onpointerdown: move |e| e.stop_propagation(),
                onclick: move |e| {
                    e.stop_propagation();
                    let before = workflow.peek().clone();
                    history.write().record(&before);
                    canvas::set_edge(&mut workflow.write(), &target, None);
                },
            }
            text { class: "cut-x", x: "{mx}", y: "{my}", "×" }
        }
    }
}

#[derive(Clone, Copy)]
struct Handlers {
    workflow: Signal<Workflow>,
    history: Signal<History>,
    drag: Signal<Drag>,
    selected: Signal<Option<String>>,
    mount: Signal<Option<std::rc::Rc<MountedData>>>,
    frame: Signal<Frame4>,
}

#[allow(clippy::too_many_arguments)]
fn card(
    node: &Node,
    current: &Workflow,
    live: &Drag,
    is_selected: bool,
    state: Option<RunState>,
    l: &kortex_i18n::Catalog,
    lang: &str,
    h: Handlers,
) -> Element {
    let Handlers {
        mut workflow,
        mut history,
        mut drag,
        mut selected,
        mount,
        mut frame,
    } = h;
    let (inputs, outputs) = node_ports(node, ports());
    let height = canvas::node_height(node, ports());
    let pos = node.pos.unwrap_or(crate::engine::Pos { x: 0, y: 0 });
    let title = node_title(node, l, lang);
    let id = node.id.clone();
    let kind = if matches!(node.kind, NodeKind::Text { .. }) {
        "text"
    } else {
        "tool"
    };
    let rows = inputs.len().max(outputs.len()).max(1);
    let state_name = match state {
        Some(RunState::Running) => "running",
        Some(RunState::Done) => "done",
        Some(RunState::Failed) => "failed",
        Some(RunState::Waiting) => "waiting",
        None => "",
    };
    let preview = match &node.kind {
        NodeKind::Text { value } => Some(if value.is_empty() {
            "…".to_owned()
        } else {
            value.replace('\n', " ")
        }),
        NodeKind::Tool { .. } => None,
    };
    let row_y = |i: usize| {
        canvas::HEADER_H + canvas::BODY_PAD + i as f64 * canvas::ROW_H + canvas::ROW_H / 2.0
    };
    let linking = if let Drag::Link { from } = live {
        Some(from.clone())
    } else {
        None
    };
    let handle = |port: &PortView, output: bool, index: usize| -> Element {
        let endpoint = Endpoint {
            node: node.id.clone(),
            port: port.id.clone(),
        };
        let linked = current.edges.iter().any(|e| {
            if output {
                e.from == endpoint
            } else {
                e.to == endpoint
            }
        });
        let ok = linking.as_ref().and_then(|from| {
            (!output).then(|| can_connect(current, ports(), from, &endpoint).is_ok())
        });
        let name = format!("{}.{}", node.id, port.id);
        let ep = endpoint.clone();
        let ep_up = endpoint.clone();
        rsx! {
            div {
                key: "{name}-{output}",
                class: "flow-port",
                "data-port": "{name}",
                "data-side": if output { "out" } else { "in" },
                "data-linked": "{linked}",
                "data-ok": ok.map(|v| v.to_string()).unwrap_or_default(),
                style: "top: {row_y(index)}px;",
                role: "button",
                "aria-label": "{name}",
                onpointerdown: move |e| {
                    e.stop_propagation();
                    if output {
                        drag.set(Drag::Link { from: ep.clone() });
                        let m = mount.peek().clone();
                        spawn(async move {
                            if let Some(f) = read_frame(m).await {
                                frame.set(f);
                            }
                        });
                    }
                },
                onpointerup: move |e| {
                    if output {
                        return;
                    }
                    e.stop_propagation();
                    if let Drag::Link { from } = drag.peek().clone() {
                        let before = workflow.peek().clone();
                        let done = connect(&mut workflow.write(), ports(), &from, &ep_up).is_ok();
                        if done {
                            history.write().record(&before);
                        }
                    }
                    drag.set(Drag::Idle);
                },
            }
        }
    };
    let down_id = id.clone();
    let focus_id = id.clone();
    let key_id = id.clone();
    let _ = key_id;
    rsx! {
        div {
            key: "{id}",
            class: "flow-node",
            "data-node": "{id}",
            "data-kind": kind,
            "data-selected": "{is_selected}",
            "data-state": state_name,
            role: "group",
            tabindex: "0",
            "aria-label": "{title} {id}",
            style: "left: {pos.x}px; top: {pos.y}px; height: {height}px;",
            onfocus: move |_| selected.set(Some(focus_id.clone())),
            onpointerdown: move |e| {
                e.stop_propagation();
                let p = e.client_coordinates();
                selected.set(Some(down_id.clone()));
                drag.set(Drag::Node {
                    id: down_id.clone(),
                    x: p.x,
                    y: p.y,
                    px: f64::from(pos.x),
                    py: f64::from(pos.y),
                    moved: false,
                });
            },
            div { class: "flow-node-head",
                span { class: "flow-badge" }
                span { class: "flow-node-title", "{title}" }
                span { class: "flow-node-id", "{id}" }
            }
            div { class: "flow-rows",
                for i in 0..rows {
                    div { key: "{i}", class: "flow-row",
                        span { class: if inputs.get(i).is_some_and(|p| p.required) { "in req" } else { "in" },
                            "{inputs.get(i).map(|p| p.id.as_str()).unwrap_or(\"\")}"
                        }
                        span { class: "out", "{outputs.get(i).map(|p| p.id.as_str()).unwrap_or(\"\")}" }
                    }
                }
            }
            if let Some(text) = preview {
                div { class: "flow-preview", "{text}" }
            }
            for (i , port) in inputs.iter().enumerate() {
                {handle(port, false, i)}
            }
            for (i , port) in outputs.iter().enumerate() {
                {handle(port, true, i)}
            }
        }
    }
}

fn minimap(current: &Workflow, v: View, frame: Frame4) -> Element {
    let Some((x1, y1, x2, y2)) = canvas::bounds(current, ports()) else {
        return rsx! {};
    };
    let (w, h) = (frame.2, frame.3);
    let (vx, vy) = v.to_canvas(0.0, 0.0);
    let (vw, vh) = (w / v.zoom, h / v.zoom);
    let (minx, miny) = (x1.min(vx) - 40.0, y1.min(vy) - 40.0);
    let (maxx, maxy) = (x2.max(vx + vw) + 40.0, y2.max(vy + vh) + 40.0);
    rsx! {
        svg {
            class: "flow-minimap",
            "aria-hidden": "true",
            view_box: "{minx} {miny} {maxx - minx} {maxy - miny}",
            preserve_aspect_ratio: "xMidYMid meet",
            for node in current.nodes.clone() {
                if let Some(p) = node.pos {
                    rect {
                        key: "{node.id}",
                        class: "n",
                        x: "{p.x}",
                        y: "{p.y}",
                        width: "{NODE_W}",
                        height: "{canvas::node_height(&node, ports())}",
                        rx: "8",
                    }
                }
            }
            rect { class: "v", x: "{vx}", y: "{vy}", width: "{vw}", height: "{vh}" }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inspector(
    node: &Node,
    current: &Workflow,
    mut workflow: Signal<Workflow>,
    mut history: Signal<History>,
    mut selected: Signal<Option<String>>,
    results: Signal<BTreeMap<String, ValueMap>>,
    pro: bool,
    l: &'static kortex_i18n::Catalog,
    lang: &str,
    note: Signal<String>,
) -> Element {
    let id = node.id.clone();
    let title = node_title(node, l, lang);
    let retries = node.retries;
    let mut snapshot = move || {
        let before = workflow.peek().clone();
        history.write().record(&before);
    };
    let (remove_id, retry_id) = (id.clone(), id.clone());
    let produced = results.read().get(&node.id).cloned().unwrap_or_default();
    rsx! {
        strong { "{id} · {title}" }
        match node.kind.clone() {
            NodeKind::Text { value } => {
                let text_id = id.clone();
                rsx! {
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("ui.text_node")} }
                        textarea {
                            class: "k-input k-textarea",
                            rows: "6",
                            value: "{value}",
                            onfocus: move |_| snapshot(),
                            oninput: move |e| {
                                if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == text_id) {
                                    n.kind = NodeKind::Text { value: e.value() };
                                }
                            },
                        }
                    }
                }
            }
            NodeKind::Tool { tool, params } => {
                let entry = shell::find(&tool);
                let specs = ports().get(&tool).cloned().unwrap_or_default();
                let visible: Vec<_> = entry
                    .map(|e| e.params.iter().filter(|p| pro || p.level != Some(ParamLevel::Pro)).cloned().collect())
                    .unwrap_or_default();
                rsx! {
                    for input in specs.inputs.clone() {
                        {wire(&id, &input, current, workflow, history, l)}
                    }
                    for param in visible {
                        {
                            let spec = specs.params.iter().find(|s| s.id == param.id).cloned();
                            let value = params
                                .get(&param.id)
                                .cloned()
                                .or_else(|| spec.as_ref().map(|s| s.default.clone()))
                                .unwrap_or_default();
                            let label = param.label.as_ref().and_then(|m| m.get(lang).or_else(|| m.get("en"))).cloned().unwrap_or_else(|| param.id.clone());
                            let (node_id, key) = (id.clone(), param.id.clone());
                            let mut set = move |text: String| {
                                if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == node_id)
                                    && let NodeKind::Tool { params, .. } = &mut n.kind
                                {
                                    params.insert(key.clone(), text);
                                }
                            };
                            let options = spec.as_ref().map(|s| s.options.clone()).unwrap_or_default();
                            let is_bool = param.ty == PortType::Bool;
                            rsx! {
                                label { key: "{param.id}", class: "k-field",
                                    span { class: "k-muted", "{label}" }
                                    if !options.is_empty() || is_bool {
                                        select {
                                            class: "k-input",
                                            value: "{value}",
                                            onfocus: move |_| snapshot(),
                                            onchange: move |e| set(e.value()),
                                            for option in if is_bool { vec!["true".to_owned(), "false".to_owned()] } else { options } {
                                                option { key: "{option}", value: "{option}", "{option}" }
                                            }
                                        }
                                    } else {
                                        input {
                                            class: "k-input",
                                            value: "{value}",
                                            onfocus: move |_| snapshot(),
                                            oninput: move |e| set(e.value()),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        label { class: "k-field",
            span { class: "k-muted", {l.t("ui.retries")} }
            select {
                class: "k-input",
                value: "{retries}",
                onchange: move |e| {
                    snapshot();
                    if let Some(n) = workflow.write().nodes.iter_mut().find(|n| n.id == retry_id) {
                        n.retries = e.value().parse().unwrap_or(0);
                    }
                },
                for n in 0..=3_u8 {
                    option { key: "{n}", value: "{n}", "{n}" }
                }
            }
        }
        if !produced.is_empty() {
            strong { {l.t("ui.result")} }
            for (port , value) in produced {
                div { key: "{port}",
                    span { class: "k-muted", "{port}" }
                    div { class: "flow-result", "{summary(&value)}" }
                    if let Value::File(file) = value.clone() {
                        button {
                            class: "k-btn",
                            "data-variant": "ghost",
                            r#type: "button",
                            onclick: move |_| save_file(&file.name, &file.mime, &file.bytes, note, l),
                            {l.t("ui.download")}
                        }
                    }
                }
            }
        }
        button {
            class: "k-btn",
            "data-variant": "ghost",
            r#type: "button",
            "data-remove": "{remove_id}",
            onclick: move |_| {
                snapshot();
                canvas::remove_node(&mut workflow.write(), &remove_id);
                selected.set(None);
            },
            {l.t("ui.remove")}
        }
    }
}

/// Keyboard-friendly alternative to dragging: pick the upstream output of an input from a list.
fn wire(
    node_id: &str,
    input: &crate::engine::Port,
    current: &Workflow,
    mut workflow: Signal<Workflow>,
    mut history: Signal<History>,
    l: &kortex_i18n::Catalog,
) -> Element {
    let target = Endpoint {
        node: node_id.to_owned(),
        port: input.id.clone(),
    };
    let chosen = current
        .edges
        .iter()
        .find(|e| e.to == target)
        .map(|e| format!("{}.{}", e.from.node, e.from.port))
        .unwrap_or_default();
    let candidates: Vec<String> = current
        .nodes
        .iter()
        .filter(|n| n.id != node_id)
        .flat_map(|n| {
            node_ports(n, ports())
                .1
                .into_iter()
                .filter(|p| {
                    can_connect(
                        current,
                        ports(),
                        &Endpoint {
                            node: n.id.clone(),
                            port: p.id.clone(),
                        },
                        &target,
                    )
                    .is_ok()
                })
                .map(|p| format!("{}.{}", n.id, p.id))
                .collect::<Vec<_>>()
        })
        .collect();
    let label = format!(
        "{} ← {}",
        input.id,
        if input.required {
            l.t("ui.required")
        } else {
            l.t("ui.optional")
        }
    );
    rsx! {
        label { key: "in-{input.id}", class: "k-field",
            span { class: "k-muted", "{label}" }
            select {
                class: "k-input",
                value: "{chosen}",
                onchange: move |e| {
                    let before = workflow.peek().clone();
                    history.write().record(&before);
                    let from = e.value().split_once('.').map(|(node, port)| Endpoint { node: node.to_owned(), port: port.to_owned() });
                    canvas::set_edge(&mut workflow.write(), &target, from);
                },
                option { value: "", "—" }
                for candidate in candidates {
                    option { key: "{candidate}", value: "{candidate}", "{candidate}" }
                }
            }
        }
    }
}
