//! Desktop player: a Dioxus window around the generated interface, a `wasmi` host for the tool's
//! engine, and the headless `--engine` service. The tool is the payload written into this binary
//! (see `kortex-payload`); a bare player has none and says so.
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Duration;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use kortex_engine_host::EngineModule;
use kortex_runtime::{CancelToken, FileData, ToolError, ValueMap};
use kortex_ui::{AutoTool, RunFuture, Runner, SaveFile};

use crate::CSS;
use crate::tool::{Package, Tool};

static TOOL: OnceLock<Tool> = OnceLock::new();
/// The tool's `engine.wasm`, compiled once on first use (see [`module`]).
static WASM: OnceLock<Vec<u8>> = OnceLock::new();
static MODULE: OnceLock<Result<EngineModule, String>> = OnceLock::new();

/// The compiled engine. Compilation takes a few hundred milliseconds: `main` starts it in the
/// background so the window opens at once, and whoever needs the engine first waits for it.
fn module() -> Result<&'static EngineModule, ToolError> {
    MODULE
        .get_or_init(|| match WASM.get() {
            Some(wasm) => EngineModule::load(wasm),
            None => Err("engine not loaded".to_owned()),
        })
        .as_ref()
        .map_err(|e| ToolError::with_detail("worker_failed", e.clone()))
}

fn timeout() -> Option<Duration> {
    TOOL.get()
        .map(|tool| Duration::from_millis(u64::from(tool.timeout_ms())))
}

/// Blocking engine entry shared by the window and `--engine`; always runs off the UI thread.
fn engine(inputs: ValueMap, params: ValueMap, cancel: CancelToken) -> Result<ValueMap, ToolError> {
    module()?.run(&inputs, &params, &cancel, timeout())
}

fn runner(inputs: ValueMap, params: ValueMap) -> RunFuture {
    Box::pin(kortex_runtime::native::run_isolated(
        inputs,
        params,
        timeout(),
        engine,
    ))
}

/// Saves a user-requested output through the native platform backend.
fn save(file: &FileData) -> Result<String, String> {
    kortex_platform::native::save_download(&file.name, &file.bytes)
}

fn load() -> Result<Package, String> {
    let payload = kortex_payload::embedded().ok_or(
        "lecteur sans outil : il faut l'assembler avec un paquet (`cargo xtask build-desktop <id>`)",
    )?;
    let files = kortex_payload::decode(payload).map_err(|e| e.to_string())?;
    Package::from_files(&files)
}

pub fn main() -> ExitCode {
    let package = match load() {
        Ok(package) => package,
        Err(message) => {
            kortex_platform::native::report(&message);
            return ExitCode::from(2);
        }
    };
    let _ = TOOL.set(package.tool);
    let _ = WASM.set(package.engine);
    // Compile the engine while the window opens (and before the first `--engine` request).
    let _ = kortex_platform::native::task::spawn("kortex-engine-warmup", || {
        let _ = module();
    });
    let Some(tool) = TOOL.get() else {
        return ExitCode::from(2);
    };
    // Headless mode: JSON-RPC lines on stdin/stdout, no window (MASTER_PROMPT section 7.3).
    if kortex_platform::native::env::args()
        .iter()
        .any(|a| a == "--engine")
    {
        return ExitCode::from(kortex_runtime::native::serve_stdio(
            &tool.contract_text,
            timeout(),
            engine,
        ));
    }
    let window = WindowBuilder::new()
        .with_title(tool.title())
        .with_inner_size(LogicalSize::new(620.0, 800.0));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window))
        .launch(App);
    ExitCode::SUCCESS
}

/// Language: `$KORTEX_LANG`, then the choice made in the Kortex apps on this device, then the
/// system locale when it is set (terminal launches); French otherwise, like the apps.
fn language() -> String {
    let env = |key: &str| {
        kortex_platform::native::env::var(key)
            .ok()
            .filter(|v| !v.is_empty())
    };
    let pick = |v: &str| if v.starts_with("en") { "en" } else { "fr" };
    if let Some(v) = env("KORTEX_LANG") {
        return pick(&v).to_owned();
    }
    if let Ok(Some(v)) = kortex_platform::document::preference_get("shell", "lang")
        && let Ok(v) = serde_json::from_str::<String>(&v)
    {
        return pick(&v).to_owned();
    }
    env("LC_ALL")
        .or_else(|| env("LANG"))
        .map_or_else(|| "fr".to_owned(), |v| pick(&v).to_owned())
}

#[component]
fn App() -> Element {
    let lang = use_hook(language);
    let Some(tool) = TOOL.get() else {
        return rsx! {};
    };
    let contract = tool.contract.clone();
    let locale = kortex_i18n::Catalog::from_toml(tool.locale_source(&lang)).unwrap_or_default();
    // Same display preferences as the Kortex apps on this device (theme, mode, text size).
    let mode = kortex_platform::native::env::var("KORTEX_MODE")
        .ok()
        .or_else(|| kortex_platform::document::shared_get("mode"));
    use_effect(move || {
        kortex_platform::document::apply_theme(
            &kortex_platform::document::shared_get("theme").unwrap_or_default(),
        );
        kortex_platform::document::apply_size(
            &kortex_platform::document::shared_get("size").unwrap_or_default(),
        );
    });
    let html_lang = lang.clone();
    use_effect(move || kortex_platform::document::apply_language(&html_lang));
    rsx! {
        style { "{CSS}" }
        main { style: "padding: var(--k-space-4); max-width: 720px; margin: 0 auto;",
            AutoTool {
                contract,
                lang,
                locale,
                runner: Runner(runner),
                variant: None::<String>,
                host_mode: mode,
                save_file: Some(SaveFile(save)),
                on_cancel: move |_| kortex_runtime::native::cancel_current(),
            }
        }
    }
}
