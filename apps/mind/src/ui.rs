//! Kortex Mind UI, shared by web and desktop: a conversation with the model, plans the user
//! approves before anything runs, results shown from local memory, and attachments whose content
//! only leaves the device with an explicit per-attachment choice.
use std::collections::BTreeMap;
use std::sync::OnceLock;

use dioxus::prelude::*;
use kortex_ai::consent::Kind;
use kortex_ai::credits::{Credits, Status};
use kortex_ai::stream::Event;
use kortex_ai::tools;
use kortex_contract::ToolContract;
use kortex_runtime::Value;
use kortex_shell::{self as shell, Frame};
use kortex_ui::Button;

use crate::client::{self, Account};
use crate::engine::{Conversation, Entry, Next, Saved, Stop, select_tools};
use crate::markdown::{self, Block, Inline};
use crate::runner;

const TOOL_LIMIT: usize = 8;

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

/// Every tool contract, read once from the generated registry.
fn contracts() -> &'static BTreeMap<String, ToolContract> {
    static CONTRACTS: OnceLock<BTreeMap<String, ToolContract>> = OnceLock::new();
    CONTRACTS.get_or_init(|| {
        kortex_workflow::contracts_from_registry(include_str!("../../../registry/index.json"))
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.tool.id.clone(), c))
            .collect()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Busy {
    Idle,
    Asking,
    Running,
    Approval,
}

/// Everything the async steps need; signals are `Copy`, so this is too.
#[derive(Clone, Copy)]
struct Shared {
    conversation: Signal<Conversation>,
    account: Signal<Account>,
    credits: Signal<Option<Credits>>,
    live: Signal<String>,
    busy: Signal<Busy>,
    error: Signal<Option<String>>,
    tool_ids: Signal<Vec<String>>,
    lang: Signal<String>,
}

fn definitions(ids: &[String], used: &[String], lang: &str) -> Vec<serde_json::Value> {
    let mut all: Vec<&String> = ids.iter().collect();
    all.extend(used.iter().filter(|u| !ids.contains(u)));
    all.into_iter()
        .filter_map(|id| contracts().get(id))
        .map(|c| tools::definition(c, lang))
        .collect()
}

/// Asks the model until it needs the user (a plan to approve), answers, or stops.
async fn drive(mut s: Shared) {
    let Account::Ready { auth, .. } = (s.account)() else {
        s.error.set(Some("unauthorized".into()));
        return;
    };
    loop {
        s.busy.set(Busy::Asking);
        s.live.set(String::new());
        s.error.set(None);
        let request = {
            let conversation = s.conversation.read();
            let tools = definitions(&(s.tool_ids)(), &conversation.used_tools(), &(s.lang)());
            conversation.request(tools, true)
        };
        let outcome = client::ask(&auth, &request, |event| {
            if let Event::Delta {
                content: Some(text),
                ..
            } = event
            {
                s.live.write().push_str(text);
            }
        })
        .await;
        s.live.set(String::new());
        match outcome {
            Err(error) => {
                s.error.set(Some(error.code));
                s.busy.set(Busy::Idle);
                if let Some(credits) = client::credits(&auth).await {
                    s.credits.set(Some(credits));
                }
                return;
            }
            Ok((turn, credits)) => {
                if credits.is_some() {
                    s.credits.set(credits);
                }
                let next = s.conversation.write().receive(turn, contracts());
                match next {
                    Next::Ask => {}
                    Next::AwaitApproval => {
                        s.busy.set(Busy::Approval);
                        return;
                    }
                    Next::Answer | Next::Stop(_) => {
                        s.busy.set(Busy::Idle);
                        return;
                    }
                }
            }
        }
    }
}

/// Runs the approved plan locally, one call at a time, then lets the model continue.
async fn approve(mut s: Shared, confirmed: Vec<String>) {
    let calls = match s.conversation.write().approve(&confirmed) {
        Ok(calls) => calls,
        Err(missing) => {
            s.error.set(Some(format!(
                "missing_confirmation:{}:{}",
                missing.call_id, missing.kind
            )));
            return;
        }
    };
    s.error.set(None);
    s.busy.set(Busy::Running);
    for call in calls {
        let result = runner::run(&call).await;
        s.conversation.write().record_result(&call.call_id, result);
    }
    s.conversation.write().finish_round();
    drive(s).await;
}

fn output_text(value: &Value) -> String {
    match value {
        Value::Text(t) => t.clone(),
        Value::Int(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::File(f) => format!("{} ({} B)", f.name, f.bytes.len()),
        Value::Files(files) => format!("{} files", files.len()),
        Value::Table(t) => format!("{} × {}", t.rows.len(), t.columns.len()),
    }
}

fn inline_view(inline: &Inline) -> Element {
    match inline {
        Inline::Text(t) => rsx! { "{t}" },
        Inline::Bold(t) => rsx! { strong { "{t}" } },
        Inline::Italic(t) => rsx! { em { "{t}" } },
        Inline::Code(t) => rsx! { code { "{t}" } },
        Inline::Link { text, url } => {
            rsx! { a { href: "{url}", rel: "noopener noreferrer", target: "_blank", "{text}" } }
        }
    }
}

fn markdown_view(text: &str) -> Element {
    rsx! {
        for (index , block) in markdown::parse(text).into_iter().enumerate() {
            match block {
                Block::Paragraph(spans) => rsx! { p { key: "{index}", for span in spans { {inline_view(&span)} } } },
                Block::Heading(_, spans) => rsx! { h3 { key: "{index}", for span in spans { {inline_view(&span)} } } },
                Block::List(items) => rsx! {
                    ul { key: "{index}",
                        for item in items { li { for span in item { {inline_view(&span)} } } }
                    }
                },
                Block::Code { text, .. } => rsx! { pre { key: "{index}", class: "mind-code", code { "{text}" } } },
            }
        }
    }
}

fn save_file(name: &str, mime: &str, bytes: &[u8]) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        kortex_platform::document::download(name, mime, bytes);
        let _ = (name, mime);
        String::new()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = mime;
        kortex_platform::native::save_download(name, bytes).unwrap_or_else(|error| error)
    }
}

#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let (saved, _) = use_hook(|| shell::load::<Saved>("mind", "conversation"));
    let (consent_saved, _) = use_hook(|| shell::load::<bool>("mind", "consent"));
    let mut consent = use_signal(move || consent_saved);
    let mut conversation = use_signal(move || {
        if saved.lang.is_empty() {
            Conversation::new("fr")
        } else {
            Conversation::restore(saved.clone())
        }
    });
    let mut account = use_signal(|| Account::Checking);
    let mut credits = use_signal(|| None::<Credits>);
    let live = use_signal(String::new);
    let mut busy = use_signal(|| Busy::Idle);
    let error = use_signal(|| None::<String>);
    let tool_ids = use_signal(Vec::<String>::new);
    let lang_signal = use_signal(|| lang.clone());
    let mut draft = use_signal(String::new);
    let mut pasted = use_signal(String::new);
    let mut confirmed = use_signal(Vec::<String>::new);
    let mut note = use_signal(String::new);
    let shared = Shared {
        conversation,
        account,
        credits,
        live,
        busy,
        error,
        tool_ids,
        lang: lang_signal,
    };

    use_effect(use_reactive!(|lang| {
        let mut lang_signal = lang_signal;
        lang_signal.set(lang.clone());
        conversation.write().set_lang(&lang);
    }));
    use_effect(move || {
        shell::save("mind", "conversation", &conversation.read().saved());
    });
    use_effect(move || {
        shell::save("mind", "consent", &consent());
    });
    use_future(move || async move {
        let found = client::account().await;
        if let Account::Ready { auth, .. } = &found
            && let Some(c) = client::credits(auth).await
        {
            credits.set(Some(c));
        }
        account.set(found);
        // A conversation reloaded with an open question must not look like it is still running.
        busy.set(Busy::Idle);
    });

    let transcript = conversation.read().transcript().to_vec();
    let attachments: Vec<_> = conversation.read().attachments().to_vec();
    let pending = conversation.read().pending().map(<[_]>::to_vec);
    let state = busy();
    let ready = matches!(account(), Account::Ready { .. });
    let can_ask =
        ready && consent() && credits().is_none_or(|c| c.can_ask()) && state == Busy::Idle;
    let credits_line = credits().map(|c| {
        let text = match c.status {
            Status::Paused | Status::Empty => {
                let at = c
                    .back_at_utc(kortex_platform::now_ms() / 1000)
                    .unwrap_or_default();
                format!(
                    "{} {at} UTC",
                    l.t(if c.status == Status::Paused {
                        "credits.paused"
                    } else {
                        "credits.empty"
                    })
                )
            }
            Status::Low => l.t("credits.low").to_owned(),
            Status::Ok => String::new(),
        };
        (c.percent, text)
    });
    let error_text = error().map(|code| {
        if let Some(rest) = code.strip_prefix("missing_confirmation:") {
            return format!("{} ({rest})", l.t("errors.missing_confirmation"));
        }
        let key = format!("errors.{code}");
        let text = l.t(&key);
        if text == key {
            format!("{} ({code})", l.t("errors.unknown"))
        } else {
            text.to_owned()
        }
    });
    let send_message = move || {
        let text = draft();
        if !can_ask || text.trim().is_empty() {
            return;
        }
        let mut conversation = conversation;
        if conversation.write().submit(&text) {
            let (mut draft, mut tool_ids) = (draft, tool_ids);
            draft.set(String::new());
            tool_ids.set(select_tools(contracts(), &text, &lang_signal(), TOOL_LIMIT));
            spawn(drive(shared));
        }
    };

    rsx! {
        Frame { title: l.t("ui.title"), subtitle: l.t("ui.subtitle"),
            style { {include_str!("mind.css")} }
            match account() {
                Account::Checking => rsx! { p { class: "app-note", role: "status", {l.t("account.checking")} } },
                Account::SignedOut => rsx! {
                    p { class: "app-note", role: "alert",
                        {l.t(if cfg!(target_arch = "wasm32") { "account.signed_out_web" } else { "account.signed_out_desktop" })}
                        if cfg!(target_arch = "wasm32") {
                            " "
                            a { href: "{shell::url(\"root\", &lang)}compte", {l.t("account.sign_in")} }
                        }
                    }
                },
                Account::Unavailable => rsx! { p { class: "app-note", role: "alert", {l.t("account.unavailable")} } },
                Account::Ready { email, .. } => rsx! {
                    div { class: "mind-account",
                        span { class: "app-badge", "{email}" }
                        if let Some((percent , text)) = credits_line.clone() {
                            div { class: "mind-credits",
                                span { class: "k-muted", {l.t("credits.label")} }
                                div { class: "mind-meter", role: "meter", "aria-valuenow": "{percent}", "aria-valuemin": "0", "aria-valuemax": "100",
                                    div { class: "mind-meter-fill", style: "width: {percent}%" }
                                }
                                span { "{percent} %" }
                            }
                            if !text.is_empty() {
                                p { class: "app-note", role: "status", "{text}" }
                            }
                        }
                    }
                },
            }
            if ready && !consent() {
                div { class: "app-note", role: "region", "aria-label": l.t("consent.title"),
                    strong { {l.t("consent.title")} }
                    p { {l.t("consent.body")} }
                    Button { onclick: move |_| consent.set(true), {l.t("consent.accept")} }
                }
            }
            div { class: "mind-log", role: "log", "aria-live": "polite",
                if transcript.is_empty() {
                    p { class: "app-note", {l.t("ui.empty")} }
                }
                for (index , entry) in transcript.iter().cloned().enumerate() {
                    {entry_view(index, entry, conversation, l, &lang)}
                }
                if !live().is_empty() {
                    div { class: "mind-bubble mind-assistant", {markdown_view(&live())} }
                } else if state == Busy::Asking {
                    p { class: "k-muted", {l.t("ui.thinking")} }
                }
            }
            if let Some(plan) = pending.clone() {
                div { class: "mind-plan", role: "region", "aria-label": l.t("plan.title"),
                    strong { {l.t("plan.title")} }
                    p { class: "k-muted", {l.t("plan.hint")} }
                    for call in plan.iter().cloned() {
                        for kind in call.confirmations.clone() {
                            {
                                let key = format!("{}:{kind}", call.call_id);
                                let checked = confirmed().contains(&key);
                                let label = format!("{} — {}", call.tool, l.t(&format!("plan.confirm_{kind}")));
                                rsx! {
                                    label { key: "{key}", class: "mind-confirm",
                                        input {
                                            r#type: "checkbox",
                                            checked,
                                            onchange: move |_| {
                                                let mut list = confirmed.write();
                                                if let Some(at) = list.iter().position(|k| k == &key) { list.remove(at); } else { list.push(key.clone()); }
                                            },
                                        }
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flow-bar",
                        Button {
                            disabled: state != Busy::Approval,
                            onclick: move |_| {
                                let keys = confirmed();
                                confirmed.set(Vec::new());
                                spawn(approve(shared, keys));
                            },
                            {l.t("plan.approve")}
                        }
                        Button {
                            ghost: true,
                            disabled: state != Busy::Approval,
                            onclick: move |_| {
                                let next = conversation.write().refuse();
                                if next == Next::Ask {
                                    spawn(drive(shared));
                                }
                            },
                            {l.t("plan.refuse")}
                        }
                    }
                }
            }
            if let Some(text) = error_text {
                p { class: "app-note", role: "alert", "{text}" }
            }
            if !note().is_empty() {
                p { class: "app-note", role: "status", "{note}" }
            }
            if !attachments.is_empty() {
                ul { class: "mind-attachments",
                    for attachment in attachments.iter().cloned() {
                        {
                            let (kind, n) = (attachment.kind.clone(), attachment.n);
                            let (kind_remove, kind_share) = (kind.clone(), kind.clone());
                            let reference = attachment.reference(&lang);
                            let is_text = kind == Kind::Text;
                            rsx! {
                                li { key: "{reference}", class: "mind-attachment",
                                    code { "{reference}" }
                                    span { class: "k-muted", "{attachment.name}" }
                                    if is_text {
                                        label {
                                            input {
                                                r#type: "checkbox",
                                                checked: attachment.share_content,
                                                onchange: move |event| conversation.write().set_share(&kind_share, n, event.checked()),
                                            }
                                            {l.t("attach.share")}
                                        }
                                    } else {
                                        span { class: "k-muted", {l.t("attach.never_shared")} }
                                    }
                                    button {
                                        class: "k-icon-btn",
                                        r#type: "button",
                                        "aria-label": l.t("attach.remove"),
                                        onclick: move |_| conversation.write().remove_attachment(&kind_remove, n),
                                        "×"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "mind-composer",
                label { class: "k-field",
                    span { class: "k-muted", {l.t("ui.message")} }
                    textarea {
                        class: "k-input k-textarea",
                        rows: "3",
                        placeholder: l.t("ui.placeholder"),
                        value: "{draft}",
                        disabled: !ready,
                        oninput: move |event| draft.set(event.value()),
                        onkeydown: move |event| {
                            if event.key() == Key::Enter && event.modifiers().ctrl() {
                                event.prevent_default();
                                send_message();
                            }
                        },
                    }
                }
                div { class: "flow-bar",
                    Button { disabled: !can_ask || draft().trim().is_empty(), onclick: move |_| send_message(), {l.t("ui.send")} }
                    if state == Busy::Asking || state == Busy::Running {
                        Button {
                            ghost: true,
                            onclick: move |_| {
                                client::cancel();
                                runner::cancel();
                            },
                            {l.t("ui.stop")}
                        }
                    }
                    Button {
                        ghost: true,
                        disabled: conversation.read().transcript().is_empty(),
                        onclick: move |_| {
                            client::cancel();
                            conversation.set(Conversation::new(&lang_signal()));
                            busy.set(Busy::Idle);
                            note.set(String::new());
                        },
                        {l.t("ui.new")}
                    }
                    Button {
                        ghost: true,
                        onclick: move |_| match conversation.read().export(&l_title(&lang_signal())) {
                            workflow if !workflow.nodes.is_empty() => {
                                let bytes = kortex_workflow::to_json(&workflow).into_bytes();
                                let path = save_file("mind-workflow.json", "application/json", &bytes);
                                note.set(if path.is_empty() { String::new() } else { format!("{} {path}", locale(&lang_signal()).t("ui.saved")) });
                            }
                            _ => note.set(locale(&lang_signal()).t("export.nothing").to_owned()),
                        },
                        {l.t("export.button")}
                    }
                }
                details { class: "flow-import",
                    summary { {l.t("attach.title")} }
                    label { class: "k-field",
                        span { class: "k-muted", {l.t("attach.text_label")} }
                        textarea {
                            class: "k-input k-textarea",
                            rows: "4",
                            value: "{pasted}",
                            oninput: move |event| pasted.set(event.value()),
                        }
                    }
                    Button {
                        ghost: true,
                        disabled: pasted().is_empty(),
                        onclick: move |_| {
                            let text = pasted();
                            conversation.write().attach_text(&text);
                            pasted.set(String::new());
                        },
                        {l.t("attach.add_text")}
                    }
                    kortex_ui::DropZone {
                        label: l.t("attach.drop"),
                        hint: l.t("attach.drop_hint"),
                        multiple: true,
                        onfiles: move |picked: Vec<dioxus::html::FileData>| {
                            spawn(async move {
                                for file in picked {
                                    if let Ok(file) = kortex_platform::document::read_selected(&file).await {
                                        conversation.write().attach_file(&file.name, &file.mime, file.bytes);
                                    }
                                }
                            });
                        },
                    }
                }
            }
        }
    }
}

fn l_title(lang: &str) -> String {
    locale(lang).t("export.name").to_owned()
}

fn entry_view(
    index: usize,
    entry: Entry,
    conversation: Signal<Conversation>,
    l: &'static kortex_i18n::Catalog,
    lang: &str,
) -> Element {
    let tool_name =
        |id: &str| shell::find(id).map_or_else(|| id.to_owned(), |e| shell::name(e, lang));
    match entry {
        Entry::User(text) => {
            rsx! { div { key: "{index}", class: "mind-bubble mind-user", "{text}" } }
        }
        Entry::Assistant(text) => {
            rsx! { div { key: "{index}", class: "mind-bubble mind-assistant", {markdown_view(&text)} } }
        }
        Entry::Plan(lines) => {
            let rows: Vec<(String, String, String, String)> = lines
                .iter()
                .map(|line| {
                    let params = line
                        .params
                        .iter()
                        .map(|(k, v)| format!("{k} = {v}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    (
                        line.call_id.clone(),
                        format!("{} · {}", line.call_id, tool_name(&line.tool)),
                        line.inputs.join(", "),
                        params,
                    )
                })
                .collect();
            rsx! {
                div { key: "{index}", class: "mind-step",
                    strong { {l.t("plan.title")} }
                    ol {
                        for (id , title , inputs , params) in rows {
                            li { key: "{id}",
                                strong { "{title}" }
                                if !inputs.is_empty() {
                                    div { class: "k-muted", "{inputs}" }
                                }
                                if !params.is_empty() {
                                    div { class: "k-muted", "{params}" }
                                }
                            }
                        }
                    }
                }
            }
        }
        Entry::Result { call_id, tool } => {
            let outputs = conversation.read().result(&call_id).cloned();
            rsx! {
                div { key: "{index}", class: "mind-step mind-result",
                    strong { "{call_id} · {tool_name(&tool)}" }
                    if let Some(outputs) = outputs {
                        for (port , value) in outputs {
                            {
                                let text = output_text(&value);
                                let file = if let Value::File(f) = &value { Some((f.name.clone(), f.mime.clone(), f.bytes.clone())) } else { None };
                                rsx! {
                                    div { key: "{port}", class: "mind-output",
                                        span { class: "k-muted", "{port}" }
                                        pre { class: "mind-code", "{text}" }
                                        if let Some((name , mime , bytes)) = file {
                                            button {
                                                class: "k-btn",
                                                "data-variant": "ghost",
                                                r#type: "button",
                                                onclick: move |_| { let _ = save_file(&name, &mime, &bytes); },
                                                {l.t("result.download")}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        span { class: "k-muted", {l.t("result.not_kept")} }
                    }
                }
            }
        }
        Entry::Failed {
            call_id,
            tool,
            code,
            detail,
        } => {
            let key = format!("errors.{code}");
            let text = l.t(&key);
            let shown = if text == key {
                code.clone()
            } else {
                text.to_owned()
            };
            rsx! {
                div { key: "{index}", class: "mind-step app-note", role: "alert",
                    strong { "{call_id} · {tool_name(&tool)}" }
                    " — {shown}"
                    if let Some(detail) = detail { span { class: "k-muted", " ({detail})" } }
                }
            }
        }
        Entry::Refused => rsx! { p { key: "{index}", class: "k-muted", {l.t("plan.refused")} } },
        Entry::Stopped(stop) => rsx! {
            p { key: "{index}", class: "app-note", role: "status", {l.t(match stop { Stop::TooManyRounds => "errors.too_many_steps", Stop::TooManyCorrections => "errors.too_many_corrections" })} }
        },
    }
}
