use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use kortex_pkg::{PkgEnv, ToolStatus, install, installed_binary, remove, statuses, update};
use kortex_store::{Action, Catalogue, STORE_CSS, texts, universes, visible};

const CSS: &str = include_str!("../../../design/dist/kortex.css");

struct Catalog {
    items: Vec<ToolStatus>,
}

fn load(env: &PkgEnv) -> Result<Catalog, String> {
    let index = env.load_index().map_err(|e| e.to_string())?;
    let reg = env.home.load_registry().map_err(|e| e.to_string())?;
    Ok(Catalog {
        items: statuses(&index, &reg, env.target.as_deref()),
    })
}

/// Runs one install/update/remove/open on a worker thread (the package code is blocking I/O).
fn perform(env: &PkgEnv, id: &str, action: Action, lang: &str) -> Result<String, String> {
    let index = env.load_index().map_err(|e| e.to_string())?;
    let target = env.target().map_err(|e| e.to_string())?.to_owned();
    let mut silent = |_: &str| {};
    match action {
        Action::Install => install(&env.home, &index, &env.index_src, id, &target, &mut silent)
            .map(|t| format!("{id} {}", t.version))
            .map_err(|e| e.to_string()),
        Action::Update => update(&env.home, &index, &env.index_src, id, &target, &mut silent)
            .map(|t| t.map_or_else(|| id.to_owned(), |t| format!("{id} {}", t.version)))
            .map_err(|e| e.to_string()),
        Action::Remove => {
            let name = index.tools.get(id).map_or_else(
                || id.to_owned(),
                |t| {
                    t.name
                        .get(lang)
                        .or_else(|| t.name.get("en"))
                        .cloned()
                        .unwrap_or_else(|| id.to_owned())
                },
            );
            remove(&env.home, id, &name)
                .map(|()| id.to_owned())
                .map_err(|e| e.to_string())
        }
        Action::Open => {
            let bin = installed_binary(&env.home, id)
                .ok_or_else(|| format!("{id} n'est pas installé"))?;
            std::process::Command::new(&bin)
                .spawn()
                .map(|_| id.to_owned())
                .map_err(|e| e.to_string())
        }
    }
}

fn main() {
    let window = WindowBuilder::new()
        .with_title("Kortex Store")
        .with_inner_size(LogicalSize::new(820.0, 780.0));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
}

#[component]
fn App() -> Element {
    let lang = std::env::var("KORTEX_LANG")
        .ok()
        .or_else(|| std::env::var("LANG").ok())
        .filter(|l| l.starts_with("fr"))
        .map_or("en", |_| "fr")
        .to_owned();
    let t = texts(&lang);
    let env = use_hook(|| PkgEnv::from_process().map_err(|e| e.to_string()));
    let mut catalog = use_signal(|| None::<Result<Vec<ToolStatus>, String>>);
    let mut query = use_signal(String::new);
    let mut universe = use_signal(|| None::<String>);
    let mut busy = use_signal(Vec::<String>::new);
    let mut toast = use_signal(|| None::<String>);

    let reload = {
        let env = env.clone();
        move || {
            let env = env.clone();
            spawn(async move {
                let res = match env {
                    Ok(env) => tokio::task::spawn_blocking(move || load(&env).map(|c| c.items))
                        .await
                        .unwrap_or_else(|e| Err(e.to_string())),
                    Err(e) => Err(e),
                };
                catalog.set(Some(res));
            });
        }
    };
    use_hook({
        let reload = reload.clone();
        move || reload()
    });

    let act = use_callback({
        let env = env.clone();
        let lang = lang.clone();
        let reload = reload.clone();
        move |(id, action): (String, Action)| {
            let Ok(env) = env.clone() else { return };
            if busy.read().contains(&id) {
                return;
            }
            busy.write().push(id.clone());
            let (lang, reload) = (lang.clone(), reload.clone());
            spawn(async move {
                let id2 = id.clone();
                let res = tokio::task::spawn_blocking(move || perform(&env, &id2, action, &lang))
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));
                busy.write().retain(|x| *x != id);
                toast.set(Some(match res {
                    Ok(done) => format!("✓ {done}"),
                    Err(e) => format!("✗ {e}"),
                }));
                reload();
            });
        }
    });

    let body = match catalog() {
        None => rsx! { p { class: "k-muted", "{t.loading}" } },
        Some(Err(e)) => rsx! {
            p { class: "k-error", role: "alert", "{e}" }
            button { class: "k-btn", r#type: "button", onclick: { let reload = reload.clone(); move |_| reload() }, "{t.retry}" }
        },
        Some(Ok(all)) => {
            let chips = universes(&all);
            let items: Vec<ToolStatus> = visible(&all, &query(), universe().as_deref(), &lang)
                .into_iter()
                .cloned()
                .collect();
            let updatable: Vec<String> = all
                .iter()
                .filter(|s| s.update_available)
                .map(|s| s.id.clone())
                .collect();
            rsx! {
                div { class: "store-bar",
                    input { class: "k-input", r#type: "search", placeholder: "{t.search}", value: "{query}", oninput: move |e| query.set(e.value()) }
                    if !updatable.is_empty() {
                        button {
                            class: "k-btn",
                            r#type: "button",
                            onclick: move |_| {
                                for id in updatable.clone() {
                                    act.call((id, Action::Update));
                                }
                            },
                            "{t.update_all} ({updatable.len()})"
                        }
                    }
                }
                div { class: "store-chips", role: "group",
                    button { class: "store-chip", r#type: "button", "aria-pressed": "{universe().is_none()}", onclick: move |_| universe.set(None), "{t.all}" }
                    for u in chips {
                        button { key: "{u}", class: "store-chip", r#type: "button", "aria-pressed": "{universe().as_deref() == Some(u.as_str())}", onclick: { let u = u.clone(); move |_| universe.set(Some(u.clone())) }, "{u}" }
                    }
                }
                Catalogue { items, lang: lang.clone(), busy: busy(), onaction: act }
            }
        }
    };

    rsx! {
        style { "{CSS}{STORE_CSS}" }
        main { class: "store",
            header {
                h1 { "{t.title}" }
                p { class: "k-muted", "{t.subtitle}" }
            }
            {body}
        }
        if let Some(msg) = toast() {
            div { class: "store-toast", role: "status", onclick: move |_| toast.set(None), "{msg}" }
        }
    }
}
