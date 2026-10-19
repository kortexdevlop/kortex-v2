//! Host side of the Agent node: asks the model through the gateway shared with Mind, runs the tools
//! the node allows (local engines, nothing that touches the network, files or secrets) and hands
//! the answer back as text and, for the table format, a table. What is sent and how an answer is
//! read lives in `kortex_ai::agent`; consent is checked here and again by the UI before a run.
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use kortex_ai::agent::{Agent, AgentError, Format, Next, Spec, Task};
use kortex_ai::credits::Credits;
use kortex_ai_client::{Account, ask};
use kortex_contract::ToolContract;
use kortex_runtime::{Value, ValueMap};
use kortex_workflow::{Job, RunError, text_io};

static CONSENT: AtomicBool = AtomicBool::new(false);
static LANG: Mutex<String> = Mutex::new(String::new());
static CREDITS: Mutex<Option<Credits>> = Mutex::new(None);

/// The user agreed that the text of Agent nodes is sent to the model.
pub fn set_consent(agreed: bool) {
    CONSENT.store(agreed, Ordering::SeqCst);
}

/// Whether the user agreed to send Agent text to the model.
#[must_use]
pub fn consented() -> bool {
    CONSENT.load(Ordering::SeqCst)
}

/// The interface language last set.
#[must_use]
pub fn lang() -> String {
    LANG.lock().map(|l| l.clone()).unwrap_or_default()
}

/// Interface language, for the wording of the instructions sent to the model.
pub fn set_lang(lang: &str) {
    if let Ok(mut current) = LANG.lock() {
        *current = lang.to_owned();
    }
}

/// The credits the gateway reported after the last call.
pub fn last_credits() -> Option<Credits> {
    CREDITS.lock().ok().and_then(|c| *c)
}

/// Contracts of every tool, for the tool-using mode.
pub fn contracts() -> &'static BTreeMap<String, ToolContract> {
    static ALL: OnceLock<BTreeMap<String, ToolContract>> = OnceLock::new();
    ALL.get_or_init(|| {
        kortex_workflow::contracts_from_registry(include_str!("../../../registry/index.json"))
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.tool.id.clone(), c))
            .collect()
    })
}

/// Tools an agent may call: one text in, a text out, and nothing that reaches the network, writes
/// files or handles sensitive data (those need a person's confirmation, which a workflow has not).
#[must_use]
pub fn safe_tools() -> Vec<String> {
    contracts()
        .values()
        .filter(|c| {
            let p = &c.permissions;
            !p.network && !p.sensitive && p.filesystem != "write"
        })
        .filter(|c| {
            crate::catalogue::ports()
                .get(&c.tool.id)
                .is_some_and(|ports| text_io(ports).is_some())
        })
        .map(|c| c.tool.id.clone())
        .collect()
}

/// Asks the model for a plain answer (no tools), for the workflow builder.
///
/// # Errors
/// `consent_required`, `ai_signin`, the gateway's own codes, or `ai_network`.
pub async fn complete(messages: Vec<kortex_ai::chat::Message>) -> Result<String, String> {
    if !CONSENT.load(Ordering::SeqCst) {
        return Err("consent_required".into());
    }
    let Account::Ready { auth, .. } = kortex_ai_client::account().await else {
        return Err("ai_signin".into());
    };
    let request = kortex_ai::chat::Request {
        messages,
        tools: Vec::new(),
        stream: true,
        // The gateway's ceiling for one answer.
        max_tokens: 2_048,
    };
    let (turn, credits) = ask(&auth, &request, |_| {}).await.map_err(|e| {
        if e.status == 0 {
            "ai_network".to_owned()
        } else {
            e.code
        }
    })?;
    if let (Some(c), Ok(mut last)) = (credits, CREDITS.lock()) {
        *last = Some(c);
    }
    Ok(turn.content.unwrap_or_default())
}

fn failure(code: &str, detail: Option<String>) -> RunError {
    RunError {
        code: code.to_owned(),
        detail,
    }
}

fn agent_error(error: &AgentError) -> RunError {
    failure(
        error.code(),
        match error {
            AgentError::UnknownTool(tool) => Some(tool.clone()),
            _ => None,
        },
    )
}

fn text(map: &ValueMap, key: &str) -> String {
    match map.get(key) {
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

fn words(list: &str) -> Vec<String> {
    list.split([',', '\n'])
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The run described by a job.
///
/// # Errors
/// `ai_unknown_task`, `ai_unknown_format` for a value the node cannot hold.
pub fn spec_of(job: &Job, lang: &str) -> Result<Spec, RunError> {
    let task =
        Task::parse(&text(&job.params, "task")).ok_or_else(|| failure("ai_unknown_task", None))?;
    let format = Format::parse(&text(&job.params, "format"))
        .ok_or_else(|| failure("ai_unknown_format", None))?;
    Ok(Spec {
        task,
        prompt: text(&job.params, "prompt"),
        format,
        fields: words(&text(&job.params, "fields")),
        language: text(&job.params, "language"),
        source: text(&job.inputs, "source"),
        context: text(&job.inputs, "context"),
        lang: if lang == "fr" { "fr" } else { "en" }.to_owned(),
    })
}

/// Runs an Agent node.
///
/// # Errors
/// `consent_required`, `ai_signin` (no account), the gateway's own codes (`ai_paused`,
/// `user_empty`…), the agent's `ai_*` codes, or `ai_network`.
pub async fn run(job: &Job) -> Result<ValueMap, RunError> {
    if !CONSENT.load(Ordering::SeqCst) {
        return Err(failure("consent_required", None));
    }
    let lang = LANG.lock().map(|l| l.clone()).unwrap_or_default();
    let spec = spec_of(job, &lang)?;
    let tools = words(&text(&job.params, "tools"));
    let max_steps = match job.params.get("max_steps") {
        Some(Value::Int(n)) => u32::try_from(*n).unwrap_or(1),
        _ => 4,
    };
    let format = spec.format;
    let mut agent =
        Agent::new(spec, contracts(), &tools, max_steps).map_err(|e| agent_error(&e))?;
    let Account::Ready { auth, .. } = kortex_ai_client::account().await else {
        return Err(failure("ai_signin", None));
    };
    let safe = safe_tools();
    loop {
        let (turn, credits) = ask(&auth, &agent.request(), |_| {}).await.map_err(|e| {
            failure(
                if e.status == 0 {
                    "ai_network"
                } else {
                    e.code.as_str()
                },
                None,
            )
        })?;
        if let (Some(c), Ok(mut last)) = (credits, CREDITS.lock()) {
            *last = Some(c);
        }
        match agent.receive(turn).map_err(|e| agent_error(&e))? {
            Next::Done(output) => {
                let mut out = ValueMap::from([("text".to_owned(), Value::Text(output.text))]);
                if let (Format::Table, Some(table)) = (format, output.table) {
                    out.insert("table".to_owned(), Value::Table(table));
                }
                return Ok(out);
            }
            Next::Ask(_) => {}
            Next::Run(calls) => {
                for call in calls {
                    // Only tools this node allows and that are safe to run unattended.
                    let outcome = if !tools.contains(&call.tool) || !safe.contains(&call.tool) {
                        Err("tool_not_allowed".to_owned())
                    } else {
                        let tool_job = Job {
                            node: job.node.clone(),
                            tool: call.tool.clone(),
                            inputs: call.inputs.clone(),
                            params: call.params.clone(),
                        };
                        crate::runner::run_tool(&tool_job).await.map_err(|e| e.code)
                    };
                    agent.record(&call.call_id, outcome);
                }
            }
        }
    }
}
