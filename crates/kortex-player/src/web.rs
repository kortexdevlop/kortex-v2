//! Web player: the page `/<tool>/` of the tool origin. It reads the tool's files (`tool.toml`,
//! `locales/*.toml`), draws the generated interface and runs the tool's `engine.wasm` in a Web
//! Worker (`/<tool>/engine/worker.js`, which starts the generic `/engines/kx-worker.js`), so one bundle serves every tool.
use std::sync::OnceLock;

use dioxus::prelude::*;
use kortex_runtime::frame::Failure;
use kortex_runtime::{ToolError, ValueMap};
use kortex_ui::{AutoTool, RunFuture, Runner};

use crate::CSS;
use crate::tool::Tool;

struct Active {
    id: String,
    timeout_ms: u32,
}

static ACTIVE: OnceLock<Active> = OnceLock::new();

fn runner(inputs: ValueMap, params: ValueMap) -> RunFuture {
    Box::pin(async move {
        let Some(active) = ACTIVE.get() else {
            return Err(ToolError::new("worker_failed"));
        };
        let url = format!("/{}/engine/worker.js", active.id);
        kortex_platform::worker::run::<ValueMap>(&url, inputs, params, active.timeout_ms)
            .await
            .map_err(|failure| {
                Failure {
                    code: failure.code,
                    detail: failure.detail,
                }
                .into_tool_error()
            })
    })
}

pub fn main() {
    dioxus::launch(App);
}

async fn text(id: &str, name: &str) -> Result<String, String> {
    kortex_platform::public::fetch_text(&format!("/{id}/{name}"), 1 << 20)
        .await
        .map_err(|e| format!("{name} : {}", e.0))
}

async fn load() -> Result<(String, Tool), String> {
    let id = kortex_platform::document::page_tool_id()
        .ok_or_else(|| "outil introuvable dans l'adresse de la page".to_owned())?;
    let tool = Tool::new(
        text(&id, "tool.toml").await?,
        text(&id, "locales/fr.toml").await?,
        text(&id, "locales/en.toml").await?,
        String::new(),
    )?;
    Ok((id, tool))
}

#[component]
fn App() -> Element {
    let loaded = use_resource(|| async { load().await });
    let state = loaded.read().clone();
    match state {
        None => rsx! { style { "{CSS}" } },
        Some(Err(message)) => rsx! {
            style { "{CSS}" }
            main { style: "padding: var(--k-space-4); max-width: 720px; margin: 0 auto;",
                p { "{message}" }
            }
        },
        Some(Ok((id, tool))) => {
            let _ = ACTIVE.set(Active {
                id: id.clone(),
                timeout_ms: tool.timeout_ms(),
            });
            rsx! { Player { id, tool } }
        }
    }
}

#[component]
fn Player(id: String, tool: Tool) -> Element {
    use_hook(|| kortex_platform::offline::register(&format!("/{id}/sw.js")));
    // The bus script lives for the whole session; mode changes from the host arrive on its channel.
    let mut host_mode = use_signal(|| None::<String>);
    let mut host_lang = use_signal(|| None::<String>);
    use_future(move || async move {
        let mut bus = kortex_platform::document::host_bus();
        while let Ok(mode) = bus.recv::<String>().await {
            if let Some(lang) = mode
                .strip_prefix("lang:")
                .filter(|l| ["fr", "en"].contains(l))
            {
                host_lang.set(Some(lang.to_owned()));
            } else {
                host_mode.set(Some(mode));
            }
        }
    });
    let query = use_resource(|| async {
        let mut ev = kortex_platform::document::launch_query();
        ev.recv::<String>()
            .await
            .unwrap_or_else(|_| "fr||".to_owned())
    });
    // Wait for the URL query: initial param values (variant presets) are computed once.
    let Some(query) = query.read().clone() else {
        return rsx! {};
    };
    let mut parts = query.split('|');
    let query_lang = parts.next().unwrap_or("fr").to_owned();
    let lang = host_lang().unwrap_or(query_lang);
    let variant = parts.next().filter(|v| !v.is_empty()).map(str::to_owned);
    let initial_mode = parts.next().filter(|m| !m.is_empty()).map(str::to_owned);
    let locale = kortex_i18n::Catalog::from_toml(tool.locale_source(&lang)).unwrap_or_default();
    let title = tool.title();
    rsx! {
        document::Title { "{title}" }
        style { "{CSS}" }
        main { style: "padding: var(--k-space-4); max-width: 720px; margin: 0 auto;",
            AutoTool {
                contract: tool.contract.clone(),
                lang,
                locale,
                runner: Runner(runner),
                variant,
                host_mode: host_mode().or(initial_mode),
                save_file: None,
                on_cancel: move |_| kortex_platform::worker::cancel(),
            }
        }
    }
}
