//! Shared Store application: catalogue and permissions everywhere; signed installation on desktop.
use dioxus::prelude::*;
use kortex_shell::{Frame, catalogue, matches, name, url, use_appearance};
use kortex_ui::Button;

use crate::engine::{Action, Outcome, Snapshot, ToolStatus, primary_action};
use crate::ui::text;

#[derive(Clone, serde::Deserialize)]
struct Permissions {
    network: bool,
    filesystem: String,
    clipboard: String,
    sensitive: bool,
}
fn permissions(id: &str) -> Option<Permissions> {
    #[derive(serde::Deserialize)]
    struct Tool {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        tool: Tool,
        permissions: Permissions,
    }
    #[derive(serde::Deserialize)]
    struct Registry {
        tools: Vec<Entry>,
    }
    static REGISTRY: std::sync::OnceLock<Vec<Entry>> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            serde_json::from_str::<Registry>(include_str!("../../../registry/index.json"))
                .map(|r| r.tools)
                .unwrap_or_default()
        })
        .iter()
        .find(|e| e.tool.id == id)
        .map(|e| e.permissions.clone())
}

/// Loads this device's package state (always empty on the web, where nothing is installed).
async fn load_snapshot() -> Snapshot {
    #[cfg(target_arch = "wasm32")]
    {
        Snapshot::default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::delivery::background(crate::delivery::snapshot)
            .await
            .unwrap_or_default()
    }
}

/// Runs one package action off the UI thread.
async fn run_action(id: String, action: Action, password: Option<String>) -> Outcome {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (id, action, password);
        Outcome::Failed(String::new())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::delivery::background(move || {
            crate::delivery::perform(&id, action, password.as_deref())
        })
        .await
        .unwrap_or_else(|| Outcome::Failed(String::new()))
    }
}

fn label(lang: &str, action: Action) -> String {
    text(
        lang,
        match action {
            Action::Install => "install",
            Action::Update => "update",
            Action::Open => "open",
            Action::Remove => "remove",
        },
    )
}

/// Install/update/open and remove buttons of one package (desktop only).
#[component]
pub fn PackageActions(
    status: ToolStatus,
    lang: String,
    busy: bool,
    onaction: EventHandler<(String, Action)>,
) -> Element {
    let primary = primary_action(&status);
    let installed = status.installed.is_some();
    let id = status.id.clone();
    let busy_label = text(&lang, "busy");
    rsx! {
        div { class: "store-actions",
            if let Some(action) = primary {
                Button {
                    disabled: busy,
                    onclick: {
                        let id = id.clone();
                        move |_| onaction.call((id.clone(), action))
                    },
                    if busy { "{busy_label}" } else { "{label(&lang, action)}" }
                }
            } else {
                span { class: "k-muted", "{text(&lang, \"unavailable\")}" }
            }
            if installed {
                Button {
                    ghost: true,
                    disabled: busy,
                    onclick: move |_| onaction.call((id.clone(), Action::Remove)),
                    "{label(&lang, Action::Remove)}"
                }
            }
        }
    }
}

#[component]
pub fn App() -> Element {
    let appearance = use_appearance();
    let lang = (appearance.lang)();
    let fr = lang == "fr";
    let mut query = use_signal(String::new);
    let mut universe = use_signal(String::new);
    let mut selected = use_signal(|| None::<String>);
    let mut only_installed = use_signal(|| false);
    let mut snapshot = use_signal(|| None::<Snapshot>);
    let mut busy = use_signal(|| None::<String>);
    let mut notice = use_signal(|| None::<(bool, String)>);
    // Install waiting for the account password: (tool id, typed password).
    let mut confirming = use_signal(|| None::<String>);
    let mut password = use_signal(String::new);
    let is_web = cfg!(target_arch = "wasm32");
    let refresh = move || {
        spawn(async move {
            snapshot.set(Some(load_snapshot().await));
        });
    };
    use_hook(move || {
        if !is_web {
            refresh();
        }
    });
    let act = use_callback(move |(id, action, pw): (String, Action, Option<String>)| {
        let lang = (appearance.lang)();
        busy.set(Some(id.clone()));
        notice.set(None);
        spawn(async move {
            let outcome = run_action(id.clone(), action, pw).await;
            busy.set(None);
            match outcome {
                Outcome::Done => {
                    confirming.set(None);
                    password.set(String::new());
                    if action != Action::Open {
                        notice.set(Some((true, text(&lang, "done"))));
                    }
                }
                Outcome::NeedsPassword => confirming.set(Some(id)),
                Outcome::WrongPassword => {
                    password.set(String::new());
                    notice.set(Some((false, text(&lang, "wrong_password"))));
                }
                Outcome::SignedOut => notice.set(Some((false, text(&lang, "signed_out")))),
                Outcome::Failed(detail) if action == Action::Open || detail.is_empty() => {
                    notice.set(Some((false, text(&lang, "open_failed"))));
                }
                Outcome::Failed(detail) => {
                    notice.set(Some((false, format!("{} {detail}", text(&lang, "failed")))));
                }
            }
            snapshot.set(Some(load_snapshot().await));
        });
    });
    let on_package = move |(id, action): (String, Action)| act.call((id, action, None));
    let snap = snapshot.read().clone().unwrap_or_default();
    let status_of = |id: &str| snap.items.iter().find(|s| s.id == id).cloned();
    let mut universes: Vec<_> = catalogue()
        .iter()
        .map(|e| e.tool.universe.clone())
        .collect();
    universes.sort();
    universes.dedup();
    let items: Vec<_> = catalogue()
        .iter()
        .filter(|e| {
            matches(e, &query(), &lang)
                && (universe().is_empty() || e.tool.universe == universe())
                && (!only_installed()
                    || status_of(&e.tool.id).is_some_and(|s| s.installed.is_some()))
        })
        .collect();
    let apps: Vec<ToolStatus> = snap
        .items
        .iter()
        .filter(|s| s.universe == "apps" && s.id != "apps.store")
        .cloned()
        .collect();
    let installed_count = snap.items.iter().filter(|s| s.installed.is_some()).count();
    let busy_id = busy();
    let detail = selected().and_then(|id| kortex_shell::find(&id));
    let download = format!(
        "{}{}",
        url("root", &lang),
        if fr { "telecharger" } else { "download" }
    );
    rsx! {
        style { {include_str!("market.css")} }
        Frame { title:"Kortex Store", subtitle:if fr{"Vos outils. Votre appareil. Vos données."}else{"Your tools. Your device. Your data."},
            section { class:"store-hero k-card",
                p { class:"app-badge store-count", if fr{"38 outils gratuits et locaux"}else{"38 free, local tools"} }
                h2 { if fr{"Tout ce qu’il vous faut, à portée de main."}else{"Everything you need, within reach."} }
                p {class:"k-muted",if fr{"Explorez les outils, vérifiez leurs permissions et ouvrez-les sans envoyer vos fichiers."}else{"Explore tools, review their permissions and open them without uploading your files."}}
                a {class:"k-btn k-btn-ghost",href:format!("{}{}",url("root",&lang),if fr{"compte"}else{"account"}),if fr{"Gérer mon compte et mes appareils"}else{"Manage my account and devices"}}
            }
            if is_web {
                section {class:"store-delivery k-card","aria-label":"Distribution",
                    h2 {"{text(&lang, \"web_title\")}"}
                    p {class:"k-muted","{text(&lang, \"web_intro\")}"}
                    a {class:"k-btn",href:download,"{text(&lang, \"web_link\")}"}
                }
            } else {
                section {class:"store-delivery k-card","aria-label":"Distribution","data-delivery":"desktop",
                    h2 {"{text(&lang, \"desktop_title\")}"}
                    p {class:"k-muted","{text(&lang, \"desktop_intro\")}"}
                    match (&*snapshot.read(), snap.account.as_deref()) {
                        (None, _) => rsx! { p {role:"status","{text(&lang, \"loading\")}"} },
                        (Some(_), Some(email)) => rsx! { p {"{text(&lang, \"signed_in\")} ", strong {"{email}"}} },
                        (Some(_), None) => rsx! { p {role:"note","{text(&lang, \"signed_out\")}"} },
                    }
                    if let Some(error) = snap.error.as_deref() {
                        p {role:"alert","{text(&lang, \"index_error\")} {error}"}
                    }
                    p {if fr{"Installations sur cet appareil : "}else{"Installed on this device: "},"{installed_count}"}
                    if let Some((ok, message)) = notice() {
                        p {role:if ok {"status"} else {"alert"},class:if ok {"store-notice"} else {"store-notice store-notice-error"},"{message}"}
                    }
                    Button {ghost:true,onclick:move |_|refresh(),"{text(&lang, \"refresh\")}"}
                }
                if !apps.is_empty() {
                    section {class:"store-apps","aria-label":text(&lang, "apps_title"),
                        h2 {"{text(&lang, \"apps_title\")}"}
                        ul {class:"store-app-list",
                            for app in apps {
                                li {class:"k-card store-app",key:"{app.id}","data-app":app.id.clone(),
                                    div {class:"store-info",
                                        strong {"{crate::engine::display_name(&app, &lang)}"}
                                        span {class:"k-muted",
                                            match &app.installed {
                                                Some(v) if app.update_available => rsx!{"v{v} → v{app.available}"},
                                                Some(v) => rsx!{"{text(&lang, \"installed\")} · v{v}"},
                                                None => rsx!{"v{app.available}"},
                                            }
                                        }
                                    }
                                    PackageActions {busy:busy_id.as_deref()==Some(app.id.as_str()),status:app.clone(),lang:lang.clone(),onaction:on_package}
                                }
                            }
                        }
                    }
                }
            }
            section { class:"store-catalogue", "aria-label":"Catalogue",
                div {class:"store-filters",
                    input {class:"k-input",r#type:"search",value:query(),placeholder:if fr{"Rechercher parmi les 38 outils"}else{"Search all 38 tools"},"aria-label":if fr{"Rechercher un outil"}else{"Search tools"},oninput:move |e|query.set(e.value())}
                    select {class:"k-input",value:universe(),"aria-label":if fr{"Univers"}else{"Universe"},onchange:move |e|universe.set(e.value()),
                        option {value:"",if fr{"Tous les univers"}else{"All universes"}}
                        for u in universes {option {value:u.clone(),"{u}"}}
                    }
                    if !is_web {
                        label {input {r#type:"checkbox",checked:only_installed(),onchange:move |e|only_installed.set(e.checked())}if fr{"Installés sur cet appareil"}else{"Installed on this device"}}
                    }
                }
                p {class:"k-muted",role:"status","{items.len()} ",if fr{"outil(s)"}else{"tool(s)"}}
                if items.is_empty() {p {if fr{"Aucun outil ne correspond. Essayez un autre mot ou univers."}else{"No matching tools. Try another word or universe."}}}
                div {class:"store-grid",
                    for entry in items {
                        article {class:"k-card store-tool",key:"{entry.tool.id}","data-tool":entry.tool.id.clone(),
                            span {class:"app-badge store-universe","{entry.tool.universe}"}
                            h2 {"{name(entry,&lang)}"}
                            p {class:"k-muted","{entry.tool.tagline.get(&lang).cloned().unwrap_or_default()}"}
                            div {class:"store-badges",span {if fr{"Gratuit"}else{"Free"}}span {"Rust"}span {"v{entry.tool.version}"}}
                            if let Some(status) = status_of(&entry.tool.id) {
                                if let Some(version) = status.installed.as_deref() {
                                    p {if fr{"Installé sur cet appareil : "}else{"Installed on this device: "},"{version}"}
                                }
                                PackageActions {busy:busy_id.as_deref()==Some(entry.tool.id.as_str()),status:status.clone(),lang:lang.clone(),onaction:on_package}
                            }
                            Button {ghost:true,onclick:{let id=entry.tool.id.clone();move |_|selected.set(Some(id.clone()))},if fr{"Voir la fiche"}else{"View details"}}
                            a {class:"k-btn k-btn-ghost",href:format!("{}{}?open=1",url("root",&lang),entry.tool.slug.get(&lang).cloned().unwrap_or_default()),if fr{"Ouvrir sur le web"}else{"Open on the web"}}
                        }
                    }
                }
            }
            if let Some(entry)=detail {
                section {class:"k-card store-detail",role:"region","aria-label":if fr{"Fiche de l’outil"}else{"Tool details"},"data-detail":entry.tool.id.clone(),
                    Button {ghost:true,onclick:move |_|selected.set(None),if fr{"Fermer la fiche"}else{"Close details"}}
                    h2 {"{name(entry,&lang)}"}
                    p {"{entry.tool.tagline.get(&lang).cloned().unwrap_or_default()}"}
                    dl {
                        dt {if fr{"Identifiant"}else{"Identifier"}}dd {"{entry.tool.id}"}
                        dt {"Version"}dd {"{entry.tool.version}"}
                        dt {if fr{"Traitement"}else{"Processing"}}dd {if fr{"Local sur votre appareil"}else{"Locally on your device"}}
                        dt {if fr{"Hors ligne"}else{"Offline"}}dd {if entry.tool.offline {if fr{"Oui, après chargement"}else{"Yes, after loading"}}else {if fr{"Non déclaré"}else{"Not declared"}}}
                    }
                    if let Some(permission) = permissions(&entry.tool.id) {
                        h3 {if fr{"Permissions déclarées"}else{"Declared permissions"}}
                        dl {
                            dt {if fr{"Réseau"}else{"Network"}}dd {if permission.network {if fr{"Oui"}else{"Yes"}}else {if fr{"Non"}else{"No"}}}
                            dt {if fr{"Fichiers"}else{"Files"}}dd {"{permission.filesystem}"}
                            dt {if fr{"Presse-papiers"}else{"Clipboard"}}dd {"{permission.clipboard}"}
                            dt {if fr{"Confirmation sensible"}else{"Sensitive confirmation"}}dd {if permission.sensitive {if fr{"Requise"}else{"Required"}}else {if fr{"Non déclarée"}else{"Not declared"}}}
                        }
                    }
                    if let Some(status) = status_of(&entry.tool.id) {
                        PackageActions {busy:busy_id.as_deref()==Some(entry.tool.id.as_str()),status,lang:lang.clone(),onaction:on_package}
                    }
                    a {class:"k-btn k-btn-ghost",href:format!("{}{}?open=1",url("root",&lang),entry.tool.slug.get(&lang).cloned().unwrap_or_default()),if fr{"Utiliser sur le web"}else{"Use on the web"}}
                }
            }
            if let Some(id) = confirming() {
                div {class:"store-scrim",
                    form {class:"k-card store-confirm",role:"dialog","aria-modal":"true","aria-labelledby":"store-confirm-title",
                        onsubmit:{let id=id.clone();move |e: FormEvent|{e.prevent_default();act.call((id.clone(),Action::Install,Some(password())));}},
                        h2 {id:"store-confirm-title","{text(&lang, \"password_title\")}"}
                        p {class:"k-muted","{text(&lang, \"password_text\")}"}
                        label {class:"k-field",
                            span {class:"k-muted","{text(&lang, \"password_label\")}"}
                            input {class:"k-input",r#type:"password",autocomplete:"current-password",autofocus:true,value:password(),oninput:move |e|password.set(e.value())}
                        }
                        if let Some((false, message)) = notice() {
                            p {role:"alert",class:"store-notice store-notice-error","{message}"}
                        }
                        div {class:"store-actions",
                            button {class:"k-btn",r#type:"submit","data-variant":"primary",disabled:busy_id.is_some()||password().is_empty(),
                                if busy_id.is_some() {"{text(&lang, \"busy\")}"} else {"{text(&lang, \"confirm\")}"}
                            }
                            Button {ghost:true,onclick:move |_|{confirming.set(None);password.set(String::new());notice.set(None);},"{text(&lang, \"cancel\")}"}
                        }
                    }
                }
            }
        }
    }
}
