//! Kortex Flow UI: a node canvas shared by web and desktop. Tools, files and logic nodes are placed
//! as cards, chained by dragging from an output handle to an input handle (or by adding a node
//! after the selected one), and run locally node by node. All editing rules live in
//! [`crate::canvas`]; the properties panel lives in [`crate::inspector`].
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::OnceLock;

use dioxus::prelude::*;
use kortex_runtime::{CancelToken, Value, ValueMap};
use kortex_shell::{self as shell, Frame};

use crate::canvas::{
    self, History, NODE_W, PortView, RunState, States, View, can_connect, connect, fit, node_ports,
    port_point,
};
use crate::catalogue::{connector_tools, ports, usable_tools};
use crate::engine::{
    Endpoint, Execution, Node, NodeKind, Step, Workflow, auto_layout, from_json, order, to_json,
    validate,
};
use crate::inspector::inspector;

pub(crate) fn locale(lang: &str) -> &'static kortex_i18n::Catalog {
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

pub(crate) fn summary(value: &Value) -> String {
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

/// Mime patterns a file node can accept, with the locale key of their name.
pub(crate) const ACCEPTS: [(&str, &str); 4] = [
    ("*/*", "ui.accept_any"),
    ("application/pdf", "ui.accept_pdf"),
    ("image/*", "ui.accept_image"),
    ("text/*", "ui.accept_text"),
];

pub(crate) fn node_title(node: &Node, l: &kortex_i18n::Catalog, lang: &str) -> String {
    match &node.kind {
        NodeKind::Text { .. } => l.t("ui.text_node").to_owned(),
        NodeKind::Tool { tool, .. } => shell::find(tool).map_or_else(
            || {
                crate::connectors::display_name(tool, lang)
                    .or_else(|| crate::catalogue::workflow_name(tool))
                    .unwrap_or_else(|| tool.clone())
            },
            |e| shell::name(&e, lang),
        ),
        NodeKind::File { multiple, .. } => l
            .t(if *multiple {
                "ui.files_node"
            } else {
                "ui.file_node"
            })
            .to_owned(),
        NodeKind::Logic { op, .. } => l.t(&format!("logic.{op}")).to_owned(),
    }
}

/// One line under the ports of a built-in card.
fn node_line(node: &Node, l: &kortex_i18n::Catalog, chosen: Option<&Value>) -> Option<String> {
    let param = |params: &BTreeMap<String, String>, key: &str| {
        params
            .get(key)
            .cloned()
            .or_else(|| {
                crate::engine::logic_ports(match &node.kind {
                    NodeKind::Logic { op, .. } => op,
                    _ => "",
                })
                .and_then(|p| {
                    p.params
                        .into_iter()
                        .find(|s| s.id == key)
                        .map(|s| s.default)
                })
            })
            .unwrap_or_default()
    };
    match &node.kind {
        NodeKind::Text { value } => Some(if value.is_empty() {
            "…".to_owned()
        } else {
            value.replace('\n', " ")
        }),
        NodeKind::Tool { .. } => None,
        NodeKind::File { .. } => Some(match chosen {
            Some(Value::File(f)) => f.name.clone(),
            Some(Value::Files(files)) => format!("{} ✓", files.len()),
            _ => l.t("ui.no_file").to_owned(),
        }),
        NodeKind::Logic { op, params } => Some(match op.as_str() {
            "condition" | "filter" => {
                let test = param(params, "test");
                format!(
                    "{} « {} »",
                    l.t(&format!("logic.test.{test}")),
                    param(params, "value")
                )
            }
            "merge" => format!("« {} »", param(params, "separator").replace('\n', "↵")),
            "http" => {
                let url = param(params, "url");
                let host = url
                    .strip_prefix("https://")
                    .and_then(|r| r.split(['/', '?']).next())
                    .unwrap_or("…");
                format!("{} {host}", param(params, "method"))
            }
            "template" => param(params, "template").replace('\n', "↵"),
            "ai" => {
                let task = param(params, "task");
                let format = param(params, "format");
                let tools = param(params, "tools");
                format!(
                    "{} → {}{}",
                    l.t(&format!("logic.task.{task}")),
                    l.t(&format!("logic.format.{format}")),
                    if tools.trim().is_empty() { "" } else { " 🛠" }
                )
            }
            "input" | "output" => param(params, "name"),
            "foreach" => {
                let tool = param(params, "tool");
                if tool.is_empty() {
                    "…".to_owned()
                } else {
                    format!(
                        "↻ {}",
                        shell::find(&tool).map_or(tool.clone(), |e| shell::name(&e, "en"))
                    )
                }
            }
            _ => format!("{} ms", param(params, "ms")),
        }),
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
        x: f64,
        y: f64,
        rem: (f64, f64),
        moved: bool,
    },
    Link {
        from: Endpoint,
    },
    Mini,
}

/// Canvas element geometry on the screen (left, top, width, height).
pub(crate) type Frame4 = (f64, f64, f64, f64);

/// Every piece of editor state, shared by the canvas, the palette and the properties panel.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    pub workflow: Signal<Workflow>,
    pub history: Signal<History>,
    pub selected: Signal<Option<String>>,
    pub extra: Signal<BTreeSet<String>>,
    pub files: Signal<BTreeMap<String, Value>>,
    pub results: Signal<BTreeMap<String, ValueMap>>,
    pub note: Signal<String>,
    pub log: Signal<Vec<String>>,
    pub outcome: Signal<Option<bool>>,
    pub library: Signal<crate::library::Library>,
    pub current_id: Signal<Option<String>>,
    pub lib_open: Signal<bool>,
    pub lib_tab: Signal<String>,
    pub lib_search: Signal<String>,
    pub confirm_delete: Signal<Option<String>>,
    pub runs: Signal<crate::history::Runs>,
    pub triggers: Signal<crate::triggers::Triggers>,
    pub trigger_edit: Signal<Option<String>>,
    pub auto_busy: Signal<bool>,
    pub trigger_err: Signal<Option<String>>,
    pub build_text: Signal<String>,
    pub build_busy: Signal<bool>,
    pub ai_consent: Signal<bool>,
    pub build_status: Signal<Option<Result<bool, String>>>,
    pub sync_on: Signal<bool>,
    pub sync_state: Signal<crate::sync::State>,
    pub sync_status: Signal<Option<Result<crate::sync::Report, String>>>,
    pub sync_running: Signal<bool>,
    pub sync_gen: Signal<u32>,
    pub sync_pass: Signal<String>,
    pub hist_open: Signal<bool>,
    pub hist_open_run: Signal<Option<u64>>,
    pub(crate) states: Signal<States>,
    view: Signal<View>,
    drag: Signal<Drag>,
    cursor: Signal<(f64, f64)>,
    frame: Signal<Frame4>,
    mount: Signal<Option<Rc<MountedData>>>,
    mini_frame: Signal<Frame4>,
    mini_mount: Signal<Option<Rc<MountedData>>>,
    clip: Signal<Option<Workflow>>,
    dragged: Signal<Option<NodeKind>>,
}

impl Ctx {
    /// Records the state before a change as one undo step.
    pub(crate) fn snapshot(mut self) {
        let before = self.workflow.peek().clone();
        self.history.write().record(&before);
    }

    fn picked(self) -> BTreeSet<String> {
        let mut all = self.extra.peek().clone();
        all.extend(self.selected.peek().clone());
        all
    }

    pub(crate) fn select_only(mut self, id: Option<String>) {
        self.selected.set(id);
        self.extra.write().clear();
    }
}

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
#[allow(clippy::too_many_arguments)]
async fn execute(
    workflow: Workflow,
    files: BTreeMap<String, Value>,
    token: CancelToken,
    mut states: Signal<States>,
    mut results: Signal<BTreeMap<String, ValueMap>>,
    mut log: Signal<Vec<String>>,
    mut runs: Signal<crate::history::Runs>,
    lang: String,
) -> bool {
    let l = locale(&lang);
    let mut execution = match Execution::new(&workflow, ports()) {
        Ok(execution) => execution.with_files(files),
        Err(failure) => {
            log.write().push(format!(
                "✗ {}",
                ui_error(l, &failure.code, failure.detail.as_deref())
            ));
            return false;
        }
    };
    let started = kortex_platform::now_ms();
    let titles: Vec<(String, String)> = order(&workflow)
        .unwrap_or_default()
        .into_iter()
        .map(|i| {
            (
                workflow.nodes[i].id.clone(),
                node_title(&workflow.nodes[i], l, &lang),
            )
        })
        .collect();
    let mut spent: BTreeMap<String, u64> = BTreeMap::new();
    let mut ok = true;
    let mut ending: (BTreeSet<String>, BTreeSet<String>, Option<(String, String)>) =
        Default::default();
    loop {
        match execution.next(&token) {
            Step::Run(job) => {
                states.write().insert(job.node.clone(), RunState::Running);
                let began = kortex_platform::now_ms();
                let outcome = crate::runner::run(&job).await;
                *spent.entry(job.node.clone()).or_insert(0) +=
                    kortex_platform::now_ms().saturating_sub(began);
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
                ending.0 = report.results.keys().cloned().collect();
                ending.1 = report.skipped.iter().cloned().collect();
                for (id, outputs) in report.results {
                    states.write().insert(id.clone(), RunState::Done);
                    results.write().insert(id, outputs);
                }
                for id in report.skipped {
                    states.write().insert(id, RunState::Skipped);
                }
                log.write().extend(report.journal);
                break;
            }
            Step::Failed(failure) => {
                ok = false;
                ending.0 = failure.report.results.keys().cloned().collect();
                ending.1 = failure.report.skipped.iter().cloned().collect();
                ending.2 = failure.node.clone().map(|node| {
                    (
                        node,
                        format!(
                            "{}{}",
                            failure.code,
                            failure
                                .detail
                                .as_deref()
                                .map(|d| format!(" ({d})"))
                                .unwrap_or_default()
                        ),
                    )
                });
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
    let (done, skipped, failed) = ending;
    let name = if workflow.name.trim().is_empty() {
        l.t("lib.untitled").to_owned()
    } else {
        workflow.name.clone()
    };
    runs.write()
        .push(crate::history::record(&crate::history::Outcome {
            workflow: &name,
            at: started,
            total_ms: kortex_platform::now_ms().saturating_sub(started),
            order: &titles,
            ms: &spent,
            done: &done,
            skipped: &skipped,
            failed: failed.as_ref().map(|(n, e)| (n.as_str(), e.clone())),
        }));
    ok
}

async fn read_frame(mount: Option<Rc<MountedData>>) -> Option<Frame4> {
    let rect = mount?.get_client_rect().await.ok()?;
    Some((
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ))
}

/// Pans (never zooms out) so a node that landed outside the canvas comes into view.
async fn reveal(mut ctx: Ctx, id: String) {
    if let Some(f) = read_frame(ctx.mount.peek().clone()).await {
        ctx.frame.set(f);
    }
    let (_, _, w, h) = (ctx.frame)();
    let v = (ctx.view)();
    let spot = ctx
        .workflow
        .peek()
        .nodes
        .iter()
        .find(|n| n.id == id)
        .and_then(|n| n.pos);
    if let Some(p) = spot {
        let (x, y) = (f64::from(p.x) * v.zoom + v.x, f64::from(p.y) * v.zoom + v.y);
        if x < 0.0 || y < 0.0 || x + NODE_W * v.zoom > w || y + 130.0 * v.zoom > h {
            ctx.view.set(canvas::centered_on(
                v,
                f64::from(p.x) + NODE_W / 2.0,
                f64::from(p.y) + 60.0,
                w,
                h,
            ));
        }
    }
}

fn add_node(mut ctx: Ctx, kind: NodeKind, at: Option<(i32, i32)>) {
    ctx.snapshot();
    let after = ctx.selected.peek().clone();
    let id = canvas::add_node(
        &mut ctx.workflow.write(),
        ports(),
        kind,
        after.as_deref(),
        at,
    );
    ctx.select_only(Some(id.clone()));
    ctx.states.write().clear();
    spawn(reveal(ctx, id));
}

fn delete_selection(mut ctx: Ctx) {
    let chosen = ctx.picked();
    if chosen.is_empty() {
        return;
    }
    ctx.snapshot();
    canvas::remove_nodes(&mut ctx.workflow.write(), &chosen);
    ctx.select_only(None);
    ctx.states.write().clear();
}

fn paste(mut ctx: Ctx) {
    let clip = ctx.clip.peek().clone();
    let Some(clip) = clip else { return };
    ctx.snapshot();
    let ids = canvas::paste_nodes(&mut ctx.workflow.write(), &clip, (48, 48));
    // The next paste of the same clipboard lands further along instead of on top.
    let mut shifted = clip;
    for node in &mut shifted.nodes {
        if let Some(p) = &mut node.pos {
            p.x += 48;
            p.y += 48;
        }
    }
    ctx.clip.set(Some(shifted));
    ctx.select_only(ids.first().cloned());
    ctx.extra.set(ids.iter().skip(1).cloned().collect());
}

fn undo(mut ctx: Ctx) {
    let now = ctx.workflow.peek().clone();
    let previous = ctx.history.write().undo(&now);
    if let Some(previous) = previous {
        ctx.workflow.set(previous);
    }
}

fn redo(mut ctx: Ctx) {
    let now = ctx.workflow.peek().clone();
    let next = ctx.history.write().redo(&now);
    if let Some(next) = next {
        ctx.workflow.set(next);
    }
}

/// Syncs the library now (if turned on) and merges the outcome into what is on screen.
pub(crate) async fn do_sync(mut ctx: Ctx) {
    if *ctx.sync_running.peek() {
        return;
    }
    let Some(passphrase) = crate::secrets::sync_passphrase() else {
        return;
    };
    ctx.sync_running.set(true);
    let mut synced = ctx.library.peek().clone();
    let mut state = ctx.sync_state.peek().clone();
    match crate::sync_net::run(&passphrase, &mut synced, &mut state).await {
        Ok(report) => {
            // The library may have changed while the network answered: keep the newest of each.
            let mut merged = ctx.library.peek().clone();
            merged.merge(&synced, kortex_platform::now_ms());
            if merged != *ctx.library.peek() {
                ctx.library.set(merged);
            }
            ctx.sync_state.set(state);
            ctx.sync_status.set(Some(Ok(report)));
        }
        Err(error) => ctx.sync_status.set(Some(Err(error.code().to_owned()))),
    }
    ctx.sync_running.set(false);
}

/// Puts another workflow in the editor as one undo step; `id` links it to a saved workflow. A
/// workflow that was never saved is kept in the library first, so opening something never loses it.
pub(crate) fn load_workflow(
    mut ctx: Ctx,
    mut workflow: Workflow,
    id: Option<String>,
    untitled: &str,
) {
    let before = ctx.workflow.peek().clone();
    if !before.nodes.is_empty() && ctx.current_id.peek().is_none() {
        let name = if before.name.trim().is_empty() {
            untitled.to_owned()
        } else {
            before.name.clone()
        };
        let kept = ctx.library.write().save(
            None,
            &name,
            &before,
            kortex_platform::now_ms(),
            crate::library::new_id,
        );
        ctx.current_id.set(Some(kept));
    }
    ctx.snapshot();
    auto_layout(&mut workflow);
    ctx.workflow.set(workflow);
    ctx.current_id.set(id);
    ctx.select_only(None);
    ctx.states.write().clear();
    ctx.results.write().clear();
    ctx.log.write().clear();
    ctx.outcome.set(None);
    ctx.files.write().clear();
    show_all(ctx);
}

fn show_all(mut ctx: Ctx) {
    spawn(async move {
        if let Some(f) = read_frame(ctx.mount.peek().clone()).await {
            ctx.frame.set(f);
        }
        let (_, _, w, h) = (ctx.frame)();
        ctx.view.set(fit(&ctx.workflow.peek(), ports(), w, h));
    });
}

#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    // Fetches the signed catalogue once and re-renders when a newer verified one arrives.
    let _ = shell::use_catalogue();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let pro = (appearance.mode)() == "pro";
    let (loaded, storage_error) = use_hook(|| shell::load::<Workflow>("flow", "workflow"));
    let workflow = use_signal(move || {
        let mut w = loaded.clone();
        auto_layout(&mut w);
        w
    });
    let (library_loaded, _) =
        use_hook(|| shell::load::<crate::library::Library>("flow", "library"));
    let (current_loaded, _) = use_hook(|| shell::load::<Option<String>>("flow", "current-id"));
    let (runs_loaded, _) = use_hook(|| shell::load::<crate::history::Runs>("flow", "runs"));
    let (sync_loaded, _) = use_hook(|| shell::load::<crate::sync::State>("flow", "sync-state"));
    let (consent_loaded, _) = use_hook(|| shell::load::<bool>("flow", "ai-consent"));
    let (triggers_loaded, _) =
        use_hook(|| shell::load::<crate::triggers::Triggers>("flow", "triggers"));
    let ctx = Ctx {
        workflow,
        history: use_signal(History::default),
        selected: use_signal(|| None),
        extra: use_signal(BTreeSet::new),
        files: use_signal(BTreeMap::new),
        results: use_signal(BTreeMap::new),
        note: use_signal(String::new),
        log: use_signal(Vec::new),
        outcome: use_signal(|| None),
        library: use_signal(move || library_loaded.clone()),
        current_id: use_signal(move || current_loaded.clone()),
        lib_open: use_signal(|| false),
        lib_tab: use_signal(|| "mine".to_owned()),
        lib_search: use_signal(String::new),
        confirm_delete: use_signal(|| None),
        runs: use_signal(move || runs_loaded.clone()),
        triggers: use_signal(move || triggers_loaded.clone()),
        trigger_edit: use_signal(|| None),
        auto_busy: use_signal(|| false),
        trigger_err: use_signal(|| None),
        build_text: use_signal(String::new),
        build_busy: use_signal(|| false),
        ai_consent: use_signal(move || consent_loaded),
        build_status: use_signal(|| None),
        sync_on: use_signal(|| crate::secrets::sync_passphrase().is_some()),
        sync_state: use_signal(move || sync_loaded.clone()),
        sync_status: use_signal(|| None),
        sync_running: use_signal(|| false),
        sync_gen: use_signal(|| 0),
        sync_pass: use_signal(String::new),
        hist_open: use_signal(|| false),
        hist_open_run: use_signal(|| None),
        view: use_signal(View::default),
        drag: use_signal(|| Drag::Idle),
        cursor: use_signal(|| (0.0, 0.0)),
        frame: use_signal(|| (0.0, 0.0, 800.0, 560.0)),
        mount: use_signal(|| None),
        mini_frame: use_signal(|| (0.0, 0.0, 168.0, 112.0)),
        mini_mount: use_signal(|| None),
        states: use_signal(States::new),
        clip: use_signal(|| None),
        dragged: use_signal(|| None),
    };
    let Ctx {
        mut view,
        mut drag,
        mut cursor,
        mut frame,
        mut mount,
        mini_frame,
        mut states,
        mut dragged,
        ..
    } = ctx;
    let (mut workflow, history, mut selected, mut extra) =
        (ctx.workflow, ctx.history, ctx.selected, ctx.extra);
    let (results, mut note, files, mut clip) = (ctx.results, ctx.note, ctx.files, ctx.clip);
    let mut import_text = use_signal(String::new);
    let mut secret_names = use_signal(crate::secrets::names);
    let mut secret_name = use_signal(String::new);
    let mut secret_value = use_signal(String::new);
    let mut search = use_signal(String::new);
    let (log, mut outcome) = (ctx.log, ctx.outcome);
    let mut running = use_signal(|| false);
    let mut cancel = use_signal(CancelToken::default);
    use_effect(move || {
        shell::save("flow", "workflow", &*workflow.read());
    });
    use_effect(move || {
        shell::save("flow", "library", &*ctx.library.read());
    });
    use_effect(move || {
        shell::save("flow", "current-id", &*ctx.current_id.read());
    });
    use_effect(move || {
        shell::save("flow", "runs", &*ctx.runs.read());
    });
    use_effect(move || {
        shell::save("flow", "sync-state", &*ctx.sync_state.read());
    });
    use_effect(move || {
        shell::save("flow", "triggers", &*ctx.triggers.read());
    });
    crate::scheduler::use_scheduler(ctx);
    // Encrypted sync: a few seconds after the library last changed (and at start-up), if turned on.
    use_effect(move || {
        let _ = ctx.library.read();
        if !*ctx.sync_on.read() {
            return;
        }
        let mut generation = ctx.sync_gen;
        let mine = generation.peek().wrapping_add(1);
        generation.set(mine);
        spawn(async move {
            kortex_platform::document::sleep(2500).await;
            if *ctx.sync_gen.peek() == mine {
                do_sync(ctx).await;
            }
        });
    });
    // Agent nodes: the account and credits are looked up only once a workflow has one, and the
    // text of those nodes is sent to the model only after the user agrees (kept on this device).
    let has_ai = use_memo(move || {
        workflow
            .read()
            .nodes
            .iter()
            .any(|n| matches!(&n.kind, NodeKind::Logic { op, .. } if op == "ai"))
    });
    let ai_consent = ctx.ai_consent;
    let mut ai_account = use_signal(|| kortex_ai_client::Account::Checking);
    let mut ai_credits = use_signal(|| None::<kortex_ai::credits::Credits>);
    crate::ai::set_consent(ai_consent());
    crate::ai::set_lang(&lang);
    use_effect(move || {
        if has_ai() && matches!(*ai_account.peek(), kortex_ai_client::Account::Checking) {
            spawn(async move {
                let found = kortex_ai_client::account().await;
                if let kortex_ai_client::Account::Ready { auth, .. } = &found {
                    ai_credits.set(kortex_ai_client::credits(auth).await);
                }
                ai_account.set(found);
            });
        }
    });

    let current = workflow();
    let catalogue = ports();
    let issues = validate(&current, catalogue);
    // Saved workflows with inputs or outputs are offered as cards (never the one being edited).
    let offered =
        crate::catalogue::workflow_tools(&ctx.library.read(), ctx.current_id.read().as_deref());
    let workflow_cards: Vec<(String, String)> = offered
        .iter()
        .map(|(id, name, _)| (id.clone(), name.clone()))
        .collect();
    crate::catalogue::set_workflow_tools(offered);
    let tools: Vec<(String, String)> = usable_tools(&lang)
        .into_iter()
        .chain(connector_tools(&lang))
        .chain(workflow_cards)
        .collect();
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
            NodeKind::Tool { tool, .. }
                if !tool.starts_with("connector.")
                    && !tool.starts_with("workflow.")
                    && shell::installed_binary(tool).is_none() =>
            {
                Some(tool.clone())
            }
            _ => None,
        })
        .collect();
    #[cfg(target_arch = "wasm32")]
    let missing: Vec<String> = Vec::new();
    let chosen_files = files();
    let unfilled: Vec<String> = current
        .nodes
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::File { .. }) && !chosen_files.contains_key(&n.id))
        .map(|n| n.id.clone())
        .collect();
    let ai_ready = !has_ai()
        || (ai_consent() && matches!(ai_account(), kortex_ai_client::Account::Ready { .. }));
    let can_run = issues.is_empty()
        && ai_ready
        && missing.is_empty()
        && unfilled.is_empty()
        && !running()
        && !current.nodes.is_empty();
    let v = view();
    let live = drag();
    let primary = selected();
    let picked = ctx.picked();
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
                        onfocus: move |_| ctx.snapshot(),
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
                    onclick: move |_| undo(ctx),
                    "↶"
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    title: l.t("ui.redo"),
                    "aria-label": l.t("ui.redo"),
                    disabled: !history.read().can_redo(),
                    onclick: move |_| redo(ctx),
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
                    onclick: move |_| show_all(ctx),
                    {l.t("ui.fit")}
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    disabled: current.nodes.is_empty(),
                    onclick: move |_| {
                        ctx.snapshot();
                        for node in &mut workflow.write().nodes {
                            node.pos = None;
                        }
                        auto_layout(&mut workflow.write());
                        show_all(ctx);
                    },
                    {l.t("ui.arrange")}
                }
                span { class: "flow-sep" }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    "data-action": "library",
                    "aria-expanded": "{ctx.lib_open}",
                    onclick: move |_| {
                        let mut open = ctx.lib_open;
                        open.set(!open());
                    },
                    {l.t("lib.title")}
                }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    "data-action": "history",
                    "aria-expanded": "{ctx.hist_open}",
                    onclick: move |_| {
                        let mut open = ctx.hist_open;
                        open.set(!open());
                    },
                    {l.t("hist.title")}
                }
                span { class: "flow-sep" }
                button {
                    class: "k-btn",
                    r#type: "button",
                    disabled: !can_run,
                    onclick: move |_| {
                        states.set(
                            workflow
                                .peek()
                                .nodes
                                .iter()
                                .map(|n| (n.id.clone(), RunState::Waiting))
                                .collect(),
                        );
                        let (mut results, mut log) = (results, log);
                        results.write().clear();
                        log.write().clear();
                        outcome.set(None);
                        running.set(true);
                        let token = CancelToken::default();
                        cancel.set(token.clone());
                        let (wf, chosen, lang) = (workflow.peek().clone(), files.peek().clone(), run_lang.clone());
                        spawn(async move {
                            let ok = execute(wf, chosen, token, states, results, log, ctx.runs, lang).await;
                            outcome.set(Some(ok));
                            running.set(false);
                            if let Some(c) = crate::ai::last_credits() {
                                ai_credits.set(Some(c));
                            }
                        });
                    },
                    {l.t("ui.run")}
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
                        crate::inspector::save_file("workflow.json", "application/json", &bytes, note, l);
                    },
                    {l.t("ui.export")}
                }
            }
            if (ctx.hist_open)() {
                {crate::libui::history_panel(ctx, l)}
            }
            if (ctx.lib_open)() {
                {crate::libui::library_panel(ctx, l, &lang)}
            }
            div { class: "flow-stage",
                // Palette: click adds the node after the selected one and links them; dragging
                // an entry onto the canvas places it where it is dropped.
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
                    {palette(&tools, &search(), l, ctx)}
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
                        ctx.select_only(None);
                        spawn(async move {
                            if let Some(f) = read_frame(mount.peek().clone()).await {
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
                            Drag::Node { x, y, rem, moved } => {
                                let zoom = view().zoom;
                                let (tx, ty) = ((p.x - x) / zoom + rem.0, (p.y - y) / zoom + rem.1);
                                let (dx, dy) = (tx.round(), ty.round());
                                if !moved {
                                    ctx.snapshot();
                                }
                                if dx != 0.0 || dy != 0.0 {
                                    canvas::shift_nodes(&mut workflow.write(), &ctx.picked(), dx as i32, dy as i32);
                                }
                                drag.set(Drag::Node { x: p.x, y: p.y, rem: (tx - dx, ty - dy), moved: true });
                            }
                            Drag::Mini => {
                                let (mx, my, mw, mh) = mini_frame();
                                let size = frame();
                                let region = canvas::minimap_region(&workflow.peek(), ports(), view(), (size.2, size.3));
                                if let Some(region) = region {
                                    let (cx, cy) = canvas::minimap_point(region, (mw, mh), p.x - mx, p.y - my);
                                    view.set(canvas::centered_on(view(), cx, cy, size.2, size.3));
                                }
                            }
                            _ => {}
                        }
                    },
                    onpointerup: move |_| drag.set(Drag::Idle),
                    onpointerleave: move |_| {
                        if matches!(drag(), Drag::Pan { .. } | Drag::Node { .. } | Drag::Mini) {
                            drag.set(Drag::Idle);
                        }
                    },
                    ondragover: move |e| e.prevent_default(),
                    ondrop: move |e| {
                        e.prevent_default();
                        let kind = dragged.peek().clone();
                        dragged.set(None);
                        if let Some(kind) = kind {
                            let p = e.client_coordinates();
                            let (ox, oy, _, _) = frame();
                            let (cx, cy) = view().to_canvas(p.x - ox, p.y - oy);
                            add_node(ctx, kind, Some(((cx - NODE_W / 2.0).round() as i32, (cy - 30.0).round() as i32)));
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
                            Key::Delete | Key::Backspace => delete_selection(ctx),
                            Key::Escape => {
                                ctx.select_only(None);
                                drag.set(Drag::Idle);
                            }
                            Key::Character(c) if ctrl => match c.to_lowercase().as_str() {
                                "z" => {
                                    e.prevent_default();
                                    if e.modifiers().shift() { redo(ctx) } else { undo(ctx) }
                                }
                                "y" => {
                                    e.prevent_default();
                                    redo(ctx);
                                }
                                "a" => {
                                    e.prevent_default();
                                    let ids: Vec<String> = workflow.peek().nodes.iter().map(|n| n.id.clone()).collect();
                                    selected.set(ids.first().cloned());
                                    extra.set(ids.into_iter().skip(1).collect());
                                }
                                "c" => {
                                    let chosen = ctx.picked();
                                    if !chosen.is_empty() {
                                        clip.set(Some(canvas::copy_nodes(&workflow.peek(), &chosen)));
                                    }
                                }
                                "v" => {
                                    e.prevent_default();
                                    paste(ctx);
                                }
                                _ => {}
                            },
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
                                {link(&edge, &current, ctx)}
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
                                    primary.as_deref() == Some(node.id.as_str()) || picked.contains(&node.id),
                                    states().get(&node.id).copied(),
                                    chosen_files.get(&node.id),
                                    l,
                                    &lang,
                                    ctx,
                                )
                            }
                        }
                    }
                    {minimap(&current, v, frame(), ctx)}
                }
                aside { class: "flow-inspector", "aria-label": l.t("ui.inspector"),
                    h2 { {l.t("ui.inspector")} }
                    if let Some(node) = primary.as_deref().and_then(|id| current.nodes.iter().find(|n| n.id == id)).cloned() {
                        {inspector(&node, &current, ctx, pro, l, &lang)}
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
            if has_ai() {
                {ai_panel(l, ai_consent, ai_account, ai_credits)}
            }
            if !unfilled.is_empty() {
                p { class: "app-note", role: "status", "{l.t(\"ui.files_needed\")} {unfilled.join(\", \")}" }
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
            details { class: "flow-import", "data-panel": "secrets",
                summary { {l.t("secrets.title")} }
                p { class: "flow-hint", {l.t("secrets.help")} }
                for name in secret_names() {
                    div { key: "{name}", class: "flow-secret",
                        code { "{{secret:{name}}}" }
                        button {
                            class: "k-btn",
                            "data-variant": "ghost",
                            r#type: "button",
                            "data-forget": "{name}",
                            onclick: move |_| {
                                crate::secrets::remove(&name);
                                secret_names.set(crate::secrets::names());
                            },
                            {l.t("secrets.forget")}
                        }
                    }
                }
                div { class: "flow-bar",
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("secrets.name")} }
                        input {
                            class: "k-input",
                            "data-field": "secret-name",
                            value: "{secret_name}",
                            oninput: move |e| secret_name.set(e.value()),
                        }
                    }
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("secrets.value")} }
                        input {
                            class: "k-input",
                            r#type: "password",
                            autocomplete: "off",
                            "data-field": "secret-value",
                            value: "{secret_value}",
                            oninput: move |e| secret_value.set(e.value()),
                        }
                    }
                    button {
                        class: "k-btn",
                        r#type: "button",
                        "data-action": "save-secret",
                        disabled: secret_name().is_empty() || secret_value().is_empty(),
                        onclick: move |_| {
                            match crate::secrets::set(&secret_name(), &secret_value()) {
                                Ok(()) => {
                                    secret_name.set(String::new());
                                    secret_value.set(String::new());
                                    secret_names.set(crate::secrets::names());
                                    note.set(String::new());
                                }
                                Err(code) => note.set(l.t(&format!("secrets.{code}")).to_owned()),
                            }
                        },
                        {l.t("secrets.save")}
                    }
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
                            ctx.snapshot();
                            let mut imported = imported;
                            auto_layout(&mut imported);
                            workflow.set(imported);
                            import_text.set(String::new());
                            ctx.select_only(None);
                            states.write().clear();
                            outcome.set(None);
                            note.set(String::new());
                            show_all(ctx);
                        }
                        Err(issue) => note.set(issue_text(issue.code, &issue.detail)),
                    },
                    {l.t("ui.import_apply")}
                }
            }
        }
    }
}

/// Palette entries: built-in nodes first, then the tools grouped by universe, filtered by the
/// search text. Every entry adds on click and can be dragged onto the canvas.
fn palette(
    tools: &[(String, String)],
    query: &str,
    l: &'static kortex_i18n::Catalog,
    ctx: Ctx,
) -> Element {
    let needle = query.trim().to_lowercase();
    let builtins: Vec<(String, String, NodeKind)> = std::iter::once((
        "text".to_owned(),
        l.t("ui.text_node").to_owned(),
        NodeKind::Text {
            value: String::new(),
        },
    ))
    .chain([false, true].map(|multiple| {
        (
            if multiple { "files" } else { "file" }.to_owned(),
            l.t(if multiple {
                "ui.files_node"
            } else {
                "ui.file_node"
            })
            .to_owned(),
            NodeKind::File {
                accept: "*/*".into(),
                multiple,
            },
        )
    }))
    .chain(crate::engine::LOGIC_OPS.iter().map(|op| {
        (
            format!("logic.{op}"),
            l.t(&format!("logic.{op}")).to_owned(),
            NodeKind::Logic {
                op: (*op).to_owned(),
                params: BTreeMap::new(),
            },
        )
    }))
    .filter(|(id, name, _)| {
        needle.is_empty() || name.to_lowercase().contains(&needle) || id.contains(&needle)
    })
    .collect();
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
    let mut dragged = ctx.dragged;
    let entry = move |attr: String, name: String, kind: NodeKind| -> Element {
        let (for_click, for_drag) = (kind.clone(), kind);
        rsx! {
            button {
                key: "{attr}",
                class: "flow-pick",
                "data-add": "{attr}",
                draggable: "true",
                r#type: "button",
                ondragstart: move |_| dragged.set(Some(for_drag.clone())),
                onclick: move |_| add_node(ctx, for_click.clone(), None),
                "{name}"
            }
        }
    };
    rsx! {
        if !builtins.is_empty() {
            div { class: "flow-group",
                span { class: "flow-group-name", {l.t("ui.builtin")} }
                for (attr , name , kind) in builtins {
                    {entry(attr, name, kind)}
                }
            }
        }
        for (universe , items) in groups {
            div { key: "{universe}", class: "flow-group",
                span { class: "flow-group-name", {group_label(l, &universe)} }
                for (id , name) in items {
                    {entry(id.clone(), name, NodeKind::Tool { tool: id, params: BTreeMap::new() })}
                }
            }
        }
    }
}

fn link(edge: &crate::engine::Edge, current: &Workflow, mut ctx: Ctx) -> Element {
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
                    ctx.snapshot();
                    canvas::set_edge(&mut ctx.workflow.write(), &target, None);
                },
            }
            text { class: "cut-x", x: "{mx}", y: "{my}", "×" }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn card(
    node: &Node,
    current: &Workflow,
    live: &Drag,
    is_selected: bool,
    state: Option<RunState>,
    chosen: Option<&Value>,
    l: &kortex_i18n::Catalog,
    lang: &str,
    ctx: Ctx,
) -> Element {
    let Ctx {
        mut workflow,
        mut drag,
        mut selected,
        mut extra,
        mount,
        mut frame,
        ..
    } = ctx;
    let (inputs, outputs) = node_ports(node, ports());
    let height = canvas::node_height(node, ports());
    let pos = node.pos.unwrap_or(crate::engine::Pos { x: 0, y: 0 });
    let title = node_title(node, l, lang);
    let id = node.id.clone();
    let kind = match &node.kind {
        NodeKind::Tool { .. } => "tool",
        NodeKind::Text { .. } => "text",
        NodeKind::File { .. } => "file",
        NodeKind::Logic { op, .. } if op == "ai" => "ai",
        NodeKind::Logic { .. } => "logic",
    };
    let rows = inputs.len().max(outputs.len()).max(1);
    let state_name = match state {
        Some(RunState::Running) => "running",
        Some(RunState::Done) => "done",
        Some(RunState::Failed) => "failed",
        Some(RunState::Skipped) => "skipped",
        Some(RunState::Waiting) => "waiting",
        None => "",
    };
    let preview = node_line(node, l, chosen);
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
                        spawn(async move {
                            if let Some(f) = read_frame(mount.peek().clone()).await {
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
                            let mut history = ctx.history;
                            history.write().record(&before);
                        }
                    }
                    drag.set(Drag::Idle);
                },
            }
        }
    };
    let (down_id, focus_id) = (id.clone(), id.clone());
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
            onfocus: move |_| {
                if selected.peek().as_ref() != Some(&focus_id) && !extra.peek().contains(&focus_id) {
                    ctx.select_only(Some(focus_id.clone()));
                }
            },
            onpointerdown: move |e| {
                e.stop_propagation();
                let p = e.client_coordinates();
                let m = e.modifiers();
                if m.shift() || m.ctrl() || m.meta() {
                    // Add to or remove from the selection; no drag starts.
                    let mut all = ctx.picked();
                    if !all.remove(&down_id) {
                        all.insert(down_id.clone());
                    }
                    let first = all.iter().next().cloned();
                    selected.set(first.clone());
                    extra.set(all.into_iter().filter(|i| Some(i) != first.as_ref()).collect());
                    return;
                }
                if !ctx.picked().contains(&down_id) {
                    ctx.select_only(Some(down_id.clone()));
                } else {
                    selected.set(Some(down_id.clone()));
                    extra.write().remove(&down_id);
                    // The clicked card becomes the primary one; the others stay selected.
                }
                drag.set(Drag::Node { x: p.x, y: p.y, rem: (0.0, 0.0), moved: false });
                let _ = &mut workflow;
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

/// Overview of the whole canvas; pressing or dragging on it moves the visible window.
fn minimap(current: &Workflow, v: View, size: Frame4, ctx: Ctx) -> Element {
    let Ctx {
        mut drag,
        mut mini_frame,
        mut mini_mount,
        mut view,
        ..
    } = ctx;
    let Some((minx, miny, rw, rh)) = canvas::minimap_region(current, ports(), v, (size.2, size.3))
    else {
        return rsx! {};
    };
    let (vx, vy) = v.to_canvas(0.0, 0.0);
    let (vw, vh) = (size.2 / v.zoom, size.3 / v.zoom);
    rsx! {
        svg {
            class: "flow-minimap",
            id: "flow-minimap",
            "aria-hidden": "true",
            view_box: "{minx} {miny} {rw} {rh}",
            preserve_aspect_ratio: "xMidYMid meet",
            onmounted: move |e| mini_mount.set(Some(e.data())),
            onpointerdown: move |e| {
                e.stop_propagation();
                let p = e.client_coordinates();
                let (cx, cy) = (p.x, p.y);
                let nodes = ctx.workflow.peek().clone();
                spawn(async move {
                    // Re-read the box: the page may have scrolled since it was drawn.
                    if let Some(f) = read_frame(ctx.mini_mount.peek().clone()).await {
                        mini_frame.set(f);
                    }
                    let rect = mini_frame();
                    let region = canvas::minimap_region(&nodes, ports(), view(), (size.2, size.3));
                    if let Some(region) = region {
                        let (px, py) = canvas::minimap_point(region, (rect.2, rect.3), cx - rect.0, cy - rect.1);
                        view.set(canvas::centered_on(view(), px, py, size.2, size.3));
                    }
                });
                drag.set(Drag::Mini);
            },
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

/// Consent, account and credits for the Agent nodes of the workflow.
fn ai_panel(
    l: &'static kortex_i18n::Catalog,
    mut consent: Signal<bool>,
    account: Signal<kortex_ai_client::Account>,
    credits: Signal<Option<kortex_ai::credits::Credits>>,
) -> Element {
    use kortex_ai::credits::Status;
    use kortex_ai_client::Account;
    let state = account();
    let status = credits().map(|c| {
        let key = match c.status {
            Status::Ok => "ai.credits_ok",
            Status::Low => "ai.credits_low",
            Status::Empty => "ai.credits_empty",
            Status::Paused => "ai.credits_paused",
        };
        format!("{} {}% — {}", l.t("ai.credits"), c.percent, l.t(key))
    });
    rsx! {
        div { class: "app-note flow-ai", "data-panel": "ai", role: "status",
            strong { {l.t("ai.title")} }
            p { {l.t("ai.what")} }
            label { class: "flow-consent",
                input {
                    r#type: "checkbox",
                    "data-field": "ai-consent",
                    checked: consent(),
                    onchange: move |e| {
                        let on = e.checked();
                        consent.set(on);
                        shell::save("flow", "ai-consent", &on);
                    },
                }
                span { {l.t("ai.consent")} }
            }
            match state {
                Account::Checking => rsx! { p { class: "flow-hint", {l.t("ai.checking")} } },
                Account::SignedOut => rsx! {
                    p { class: "flow-hint", role: "alert", "data-ai-account": "signed-out",
                        {l.t(if cfg!(target_arch = "wasm32") { "ai.signed_out_web" } else { "ai.signed_out_desktop" })}
                    }
                },
                Account::Unavailable => rsx! { p { class: "flow-hint", role: "alert", {l.t("ai.unavailable")} } },
                Account::Ready { email, .. } => rsx! {
                    p { class: "flow-hint", "data-ai-account": "ready", "{email}" }
                },
            }
            if let Some(status) = status {
                p { class: "flow-hint", "data-ai-credits": "", "{status}" }
            }
        }
    }
}

fn group_label(l: &kortex_i18n::Catalog, universe: &str) -> String {
    let key = format!("lib.group_{universe}");
    let text = l.t(&key);
    if text == key {
        universe.to_owned()
    } else {
        text.to_owned()
    }
}
