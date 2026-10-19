//! The library panel: "My workflows" (saved on this device) and ready-made templates.
use dioxus::prelude::*;

use crate::library::new_id;
use crate::templates;
use crate::ui::{Ctx, do_sync, load_workflow};

fn need_label(l: &kortex_i18n::Catalog, need: &str) -> String {
    match need.strip_prefix("secret:") {
        Some(name) => format!("{} {name}", l.t("lib.needs_secret")),
        None => l.t(&format!("lib.needs_{need}")).to_owned(),
    }
}

pub(crate) fn library_panel(ctx: Ctx, l: &'static kortex_i18n::Catalog, lang: &str) -> Element {
    let Ctx {
        mut library,
        mut current_id,
        mut lib_tab,
        mut lib_search,
        mut confirm_delete,
        workflow,
        ..
    } = ctx;
    let tab = lib_tab();
    let query = lib_search().trim().to_lowercase();
    let current = workflow();
    let here = current_id();
    let saved: Vec<_> = library
        .read()
        .visible()
        .into_iter()
        .filter(|s| query.is_empty() || s.name.to_lowercase().contains(&query))
        .cloned()
        .collect();
    let lang_owned = lang.to_owned();
    let shown: Vec<_> = templates::all()
        .iter()
        .filter(|t| {
            query.is_empty()
                || t.name(lang).to_lowercase().contains(&query)
                || t.description(lang).to_lowercase().contains(&query)
        })
        .collect();
    let suffix = l.t("lib.copy_suffix").to_owned();
    rsx! {
        section { class: "flow-library", "data-panel": "library", "aria-label": l.t("lib.title"),
            div { class: "flow-lib-head",
                div { class: "flow-tabs", role: "tablist",
                    for (id , label) in [("mine", "lib.mine"), ("templates", "lib.templates")] {
                        button {
                            key: "{id}",
                            class: "k-btn",
                            "data-variant": if tab == id { "" } else { "ghost" },
                            r#type: "button",
                            role: "tab",
                            "aria-selected": "{tab == id}",
                            "data-tab": "{id}",
                            onclick: move |_| {
                                lib_tab.set(id.to_owned());
                                lib_search.set(String::new());
                            },
                            {l.t(label)}
                        }
                    }
                }
                input {
                    class: "k-input",
                    r#type: "search",
                    placeholder: l.t("lib.search"),
                    "aria-label": l.t("lib.search"),
                    value: "{lib_search}",
                    oninput: move |e| lib_search.set(e.value()),
                }
            }
            if tab == "mine" {
                div { class: "flow-lib-actions",
                    button {
                        class: "k-btn",
                        r#type: "button",
                        "data-action": "save-workflow",
                        disabled: current.nodes.is_empty(),
                        onclick: {
                            let current = current.clone();
                            move |_| {
                                let name = if current.name.trim().is_empty() { l.t("lib.untitled").to_owned() } else { current.name.clone() };
                                let id = library.write().save(current_id.peek().as_deref(), &name, &current, kortex_platform::now_ms(), new_id);
                                current_id.set(Some(id));
                            }
                        },
                        {if here.is_some() { l.t("lib.update") } else { l.t("lib.save") }}
                    }
                    button {
                        class: "k-btn",
                        "data-variant": "ghost",
                        r#type: "button",
                        "data-action": "new-workflow",
                        onclick: move |_| load_workflow(ctx, crate::engine::Workflow::default(), None, l.t("lib.untitled")),
                        {l.t("lib.new")}
                    }
                }
                {sync_panel(ctx, l)}
                if saved.is_empty() {
                    p { class: "flow-hint", {l.t("lib.empty")} }
                }
                ul { class: "flow-lib-list",
                    for item in saved {
                        li { key: "{item.id}", class: "flow-lib-item", "data-saved": "{item.id}", "data-current": "{here.as_deref() == Some(item.id.as_str())}",
                            div { class: "flow-lib-main",
                                strong { "{item.name}" }
                                if crate::catalogue::workflow_ports(&item.workflow).is_some() {
                                    span { class: "app-badge", "data-card": "{item.id}", {l.t("lib.usable_as_card")} }
                                }
                                span { class: "flow-hint", "{item.workflow.nodes.len()} {l.t(\"lib.nodes\")}" }
                            }
                            div { class: "flow-lib-buttons",
                                button {
                                    class: "k-btn",
                                    r#type: "button",
                                    "data-action": "open",
                                    onclick: {
                                        let (item, ) = (item.clone(), );
                                        move |_| load_workflow(ctx, item.workflow.clone(), Some(item.id.clone()), l.t("lib.untitled"))
                                    },
                                    {l.t("lib.open")}
                                }
                                button {
                                    class: "k-btn",
                                    "data-variant": "ghost",
                                    r#type: "button",
                                    "data-action": "duplicate",
                                    onclick: {
                                        let (id, suffix) = (item.id.clone(), suffix.clone());
                                        move |_| {
                                            library.write().duplicate(&id, &suffix, kortex_platform::now_ms(), new_id);
                                        }
                                    },
                                    {l.t("lib.duplicate")}
                                }
                                button {
                                    class: "k-btn",
                                    "data-variant": "ghost",
                                    r#type: "button",
                                    "data-action": "triggers",
                                    "data-active": "{ctx.triggers.read().items.contains_key(&item.id)}",
                                    onclick: {
                                        let id = item.id.clone();
                                        move |_| {
                                            let mut edit = ctx.trigger_edit;
                                            let open = edit.peek().as_deref() == Some(id.as_str());
                                            edit.set(if open { None } else { Some(id.clone()) });
                                            let mut err = ctx.trigger_err;
                                            err.set(None);
                                        }
                                    },
                                    {l.t("trigger.button")}
                                }
                                if confirm_delete().as_deref() == Some(item.id.as_str()) {
                                    button {
                                        class: "k-btn",
                                        r#type: "button",
                                        "data-action": "confirm-delete",
                                        onclick: {
                                            let id = item.id.clone();
                                            move |_| {
                                                library.write().remove(&id, kortex_platform::now_ms());
                                                if current_id.peek().as_deref() == Some(id.as_str()) {
                                                    current_id.set(None);
                                                }
                                                confirm_delete.set(None);
                                            }
                                        },
                                        {l.t("lib.confirm")}
                                    }
                                } else {
                                    button {
                                        class: "k-btn",
                                        "data-variant": "ghost",
                                        r#type: "button",
                                        "data-action": "delete",
                                        onclick: {
                                            let id = item.id.clone();
                                            move |_| confirm_delete.set(Some(id.clone()))
                                        },
                                        {l.t("lib.delete")}
                                    }
                                }
                            }
                            if ctx.trigger_edit.read().as_deref() == Some(item.id.as_str()) {
                                {trigger_editor(ctx, l, &item.id, crate::scheduler::has_ai(&item.workflow))}
                            }
                        }
                    }
                }
            } else {
                p { class: "flow-hint", {l.t("lib.templates_help")} }
                div { class: "flow-lib-grid",
                    for template in shown {
                        article { key: "{template.id}", class: "flow-template", "data-template": "{template.id}",
                            strong { "{template.name(lang)}" }
                            p { "{template.description(lang)}" }
                            if !template.needs.is_empty() {
                                p { class: "flow-needs",
                                    for need in template.needs.clone() {
                                        span { key: "{need}", class: "app-badge", "{need_label(l, &need)}" }
                                    }
                                }
                            }
                            button {
                                class: "k-btn",
                                r#type: "button",
                                "data-action": "use-template",
                                onclick: {
                                    let (id, lang) = (template.id.clone(), lang_owned.clone());
                                    move |_| {
                                        if let Some(t) = templates::all().iter().find(|t| t.id == id) {
                                            load_workflow(ctx, t.instantiate(&lang), None, l.t("lib.untitled"));
                                            let mut open = ctx.lib_open;
                                            open.set(false);
                                        }
                                    }
                                },
                                {l.t("lib.use")}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Past runs: what each card did, how long it took and the error of a failed one; never data.
pub(crate) fn history_panel(ctx: Ctx, l: &'static kortex_i18n::Catalog) -> Element {
    let Ctx {
        mut runs,
        mut hist_open_run,
        ..
    } = ctx;
    let items = runs.read().items.clone();
    let none_yet = items.is_empty();
    let opened = hist_open_run();
    let error_text = |raw: &str| {
        let (code, detail) = raw
            .split_once(" (")
            .map_or((raw, None), |(c, d)| (c, Some(d.trim_end_matches(')'))));
        let key = format!("errors.{code}");
        let text = l.t(&key);
        let base = if text == key {
            code.to_owned()
        } else {
            text.to_owned()
        };
        detail.map_or(base.clone(), |d| format!("{base} ({d})"))
    };
    rsx! {
        section { class: "flow-library", "data-panel": "history", "aria-label": l.t("hist.title"),
            div { class: "flow-lib-head",
                strong { {l.t("hist.title")} }
                button {
                    class: "k-btn",
                    "data-variant": "ghost",
                    r#type: "button",
                    "data-action": "clear-history",
                    disabled: none_yet,
                    onclick: move |_| runs.write().clear(),
                    {l.t("hist.clear")}
                }
            }
            p { class: "flow-hint", {l.t("hist.help")} }
            if none_yet {
                p { class: "flow-hint", {l.t("hist.empty")} }
            }
            ul { class: "flow-lib-list",
                for run in items {
                    li { key: "{run.id}", class: "flow-lib-item", "data-run": "{run.id}", "data-ok": "{run.ok}",
                        div { class: "flow-lib-main",
                            strong { if run.ok { "✓ " } else { "✗ " } "{run.workflow}" }
                            span { class: "flow-hint",
                                "{crate::history::utc_label(run.at)} · {crate::history::duration_label(run.total_ms)} · {run.nodes.len()} {l.t(\"lib.nodes\")}"
                            }
                        }
                        div { class: "flow-lib-buttons",
                            button {
                                class: "k-btn",
                                "data-variant": "ghost",
                                r#type: "button",
                                "data-action": "toggle-run",
                                onclick: {
                                    let id = run.id;
                                    move |_| hist_open_run.set(if hist_open_run() == Some(id) { None } else { Some(id) })
                                },
                                {l.t("hist.details")}
                            }
                            button {
                                class: "k-btn",
                                "data-variant": "ghost",
                                r#type: "button",
                                "data-action": "forget-run",
                                onclick: {
                                    let id = run.id;
                                    move |_| runs.write().remove(id)
                                },
                                {l.t("hist.forget")}
                            }
                        }
                        if opened == Some(run.id) {
                            ol { class: "flow-run-nodes", "data-run-nodes": "{run.id}",
                                for node in run.nodes.clone() {
                                    li { key: "{node.node}", "data-state": "{node.state}",
                                        span { class: "flow-run-state", {state_label(l, &node.state)} }
                                        " {node.node} · {node.title}"
                                        if node.ms > 0 {
                                            " · {crate::history::duration_label(node.ms)}"
                                        }
                                        if let Some(error) = &node.error {
                                            div { class: "flow-run-error", role: "alert", "{error_text(error)}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn state_label(l: &kortex_i18n::Catalog, state: &str) -> String {
    l.t(&format!("hist.state_{state}")).to_owned()
}

/// Turn the encrypted sync on or off and see how it went.
fn sync_panel(ctx: Ctx, l: &'static kortex_i18n::Catalog) -> Element {
    let Ctx {
        mut sync_on,
        mut sync_pass,
        mut sync_status,
        sync_state,
        sync_running,
        ..
    } = ctx;
    let on = sync_on();
    let status = sync_status();
    let has_status = status.is_some();
    let last = sync_state.read().last_sync;
    rsx! {
        details { class: "flow-sync", "data-panel": "sync", open: !on || has_status,
            summary { {l.t("sync.title")} if on { " ✓" } }
            p { class: "flow-hint", {l.t("sync.help")} }
            if on {
                p { class: "flow-hint", "data-sync-last": "",
                    {if last == 0 { l.t("sync.never").to_owned() } else { format!("{} {}", l.t("sync.last"), crate::history::utc_label(last)) }}
                }
                div { class: "flow-lib-actions",
                    button {
                        class: "k-btn",
                        r#type: "button",
                        "data-action": "sync-now",
                        disabled: sync_running(),
                        onclick: move |_| {
                            spawn(do_sync(ctx));
                        },
                        {if sync_running() { l.t("sync.running") } else { l.t("sync.now") }}
                    }
                    button {
                        class: "k-btn",
                        "data-variant": "ghost",
                        r#type: "button",
                        "data-action": "sync-off",
                        onclick: move |_| {
                            crate::secrets::clear_sync_passphrase();
                            crate::sync_net::forget_key();
                            sync_on.set(false);
                            sync_status.set(None);
                        },
                        {l.t("sync.off")}
                    }
                }
            } else {
                label { class: "k-field",
                    span { class: "k-muted", {l.t("sync.passphrase")} }
                    input {
                        class: "k-input",
                        r#type: "password",
                        autocomplete: "off",
                        "data-field": "sync-passphrase",
                        value: "{sync_pass}",
                        oninput: move |e| sync_pass.set(e.value()),
                    }
                }
                p { class: "flow-hint", {l.t("sync.warning")} }
                button {
                    class: "k-btn",
                    r#type: "button",
                    "data-action": "sync-on",
                    disabled: sync_pass().chars().count() < 8,
                    onclick: move |_| {
                        if crate::secrets::set_sync_passphrase(&sync_pass.peek()) {
                            sync_pass.set(String::new());
                            sync_on.set(true);
                            sync_status.set(None);
                            spawn(do_sync(ctx));
                        } else {
                            sync_status.set(Some(Err("storage".to_owned())));
                        }
                    },
                    {l.t("sync.on")}
                }
            }
            match status {
                Some(Ok(report)) => rsx! {
                    p { class: "flow-hint", role: "status", "data-sync-result": "ok",
                        "{l.t(\"sync.done\")} {report.pulled} ↓ · {report.pushed} ↑"
                    }
                },
                Some(Err(code)) => rsx! {
                    p { class: "flow-hint", role: "alert", "data-sync-result": "{code}", {sync_error(l, &code)} }
                },
                None => rsx! {},
            }
        }
    }
}

fn sync_error(l: &kortex_i18n::Catalog, code: &str) -> String {
    let key = format!("sync.err_{code}");
    let text = l.t(&key);
    if text == key {
        format!("{} ({code})", l.t("sync.err_other"))
    } else {
        text.to_owned()
    }
}

fn token() -> String {
    let mut bytes = [0_u8; 16];
    let _ = kortex_platform::fill_random(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn trigger_error(l: &kortex_i18n::Catalog, code: &str) -> String {
    let key = format!("trigger.err_{code}");
    let text = l.t(&key);
    if text == key {
        code.to_owned()
    } else {
        text.to_owned()
    }
}

/// Starts a workflow by itself: every N minutes, daily, when Flow opens, or from a local webhook.
fn trigger_editor(ctx: Ctx, l: &'static kortex_i18n::Catalog, id: &str, has_ai: bool) -> Element {
    use crate::triggers::{Config, Kind, check, min_every};
    let desktop = !cfg!(target_arch = "wasm32");
    let (mut triggers, mut err) = (ctx.triggers, ctx.trigger_err);
    let config = triggers.read().items.get(id).cloned();
    let kind = config.as_ref().map(|c| c.kind.clone());
    let current = match &kind {
        None => "none",
        Some(Kind::Every { .. }) => "every",
        Some(Kind::Daily { .. }) => "daily",
        Some(Kind::Open) => "open",
        Some(Kind::Webhook { .. }) => "webhook",
    };
    let apply = {
        let id = id.to_owned();
        move |next: Kind| match check(&next, has_ai, desktop) {
            Err(code) => err.set(Some(code.to_owned())),
            Ok(()) => {
                err.set(None);
                let mut all = triggers.write();
                match all.items.get_mut(&id) {
                    Some(c) => {
                        c.kind = next;
                        c.failures = 0;
                    }
                    None => {
                        all.items
                            .insert(id.clone(), Config::new(next, kortex_platform::now_ms()));
                    }
                }
            }
        }
    };
    let pick = {
        let id = id.to_owned();
        let mut apply = apply.clone();
        move |e: FormEvent| match e.value().as_str() {
            "none" => {
                triggers.write().items.remove(&id);
                err.set(None);
            }
            "every" => apply(Kind::Every {
                minutes: min_every(has_ai, desktop).max(15),
            }),
            "daily" => apply(Kind::Daily { hour: 9, minute: 0 }),
            "open" => apply(Kind::Open),
            "webhook" => apply(Kind::Webhook {
                port: 47_821,
                token: token(),
            }),
            _ => {}
        }
    };
    let status = config.as_ref().and_then(|c| c.last.clone());
    let failures = config.as_ref().map_or(0, |c| c.failures);
    let enabled = config.as_ref().is_some_and(|c| c.enabled);
    let listening = crate::scheduler::listening(id);
    rsx! {
        div { class: "flow-triggers", "data-panel": "triggers",
            p { class: "flow-hint", {l.t("trigger.help")} }
            label { class: "k-field",
                span { class: "k-muted", {l.t("trigger.kind")} }
                select { class: "k-input", "data-field": "trigger-kind", value: "{current}", onchange: pick,
                    option { value: "none", selected: current == "none", {l.t("trigger.none")} }
                    option { value: "every", selected: current == "every", {l.t("trigger.every")} }
                    option { value: "daily", selected: current == "daily", {l.t("trigger.daily")} }
                    option { value: "open", selected: current == "open", {l.t("trigger.open")} }
                    if desktop {
                        option { value: "webhook", selected: current == "webhook", {l.t("trigger.webhook")} }
                    }
                }
            }
            match kind.clone() {
                Some(Kind::Every { minutes }) => rsx! {
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("trigger.minutes")} }
                        input {
                            class: "k-input",
                            r#type: "number",
                            min: "{min_every(has_ai, desktop)}",
                            "data-field": "trigger-minutes",
                            value: "{minutes}",
                            onchange: {
                                let mut apply = apply.clone();
                                move |e| {
                                    if let Ok(m) = e.value().trim().parse::<u32>() {
                                        apply(Kind::Every { minutes: m });
                                    }
                                }
                            },
                        }
                    }
                    if has_ai {
                        p { class: "flow-hint", {l.t("trigger.ai_min")} }
                    }
                },
                Some(Kind::Daily { hour, minute }) => rsx! {
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("trigger.time")} }
                        input {
                            class: "k-input",
                            r#type: "time",
                            "data-field": "trigger-time",
                            value: "{hour:02}:{minute:02}",
                            onchange: {
                                let mut apply = apply.clone();
                                move |e| {
                                    let value = e.value();
                                    if let Some((h, m)) = value.split_once(':')
                                        && let (Ok(hour), Ok(minute)) = (h.parse::<u8>(), m.parse::<u8>())
                                    {
                                        apply(Kind::Daily { hour, minute });
                                    }
                                }
                            },
                        }
                    }
                },
                Some(Kind::Webhook { port, token }) => rsx! {
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("trigger.port")} }
                        input {
                            class: "k-input",
                            r#type: "number",
                            min: "1024",
                            max: "65535",
                            "data-field": "trigger-port",
                            value: "{port}",
                            onchange: {
                                let (mut apply, token) = (apply.clone(), token.clone());
                                move |e| {
                                    if let Ok(p) = e.value().trim().parse::<u16>() {
                                        apply(Kind::Webhook { port: p.max(1024), token: token.clone() });
                                    }
                                }
                            },
                        }
                    }
                    p { class: "flow-hint", {l.t("trigger.webhook_help")} }
                    if let Some(url) = config.as_ref().and_then(Config::webhook_url) {
                        code { "data-trigger-url": "", "{url}" }
                    }
                    p { class: "flow-hint", "data-trigger-listening": "{listening}",
                        {if listening { l.t("trigger.listening") } else { l.t("trigger.not_listening") }}
                    }
                },
                _ => rsx! {},
            }
            if config.is_some() {
                label { class: "flow-check",
                    input {
                        r#type: "checkbox",
                        "data-field": "trigger-enabled",
                        checked: enabled,
                        onchange: {
                            let id = id.to_owned();
                            move |e| {
                                if let Some(c) = triggers.write().items.get_mut(&id) {
                                    c.enabled = e.checked();
                                    c.failures = 0;
                                }
                            }
                        },
                    }
                    " "
                    {l.t("trigger.enabled")}
                }
                p { class: "flow-hint", {l.t("trigger.open_only")} }
            }
            if let Some(last) = status {
                p { class: "flow-hint", "data-trigger-last": "{last.ok}", role: "status",
                    {format!("{} {} — {}", l.t("trigger.last"), crate::history::utc_label(last.at), if last.ok { l.t("trigger.ok").to_owned() } else { last.error.unwrap_or_default() })}
                }
            }
            if config.is_some() && !enabled && failures >= crate::triggers::MAX_FAILURES {
                p { class: "flow-hint", role: "alert", "data-trigger-off": "", {l.t("trigger.auto_off")} }
            }
            if let Some(code) = err() {
                p { class: "flow-hint", role: "alert", "data-trigger-error": "{code}", {trigger_error(l, &code)} }
            }
        }
    }
}
