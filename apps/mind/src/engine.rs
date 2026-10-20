//! Kortex Mind conversation rules. Pure: no UI, no network, no platform access.
//!
//! One user message starts a loop: the model answers with text or with tool calls; tool calls are
//! validated against the contracts (references resolved locally, ADR 0023), shown to the user as
//! a plan, run locally only after approval, and their *summaries* go back to the model, which
//! decides the next step. This module owns that state machine; hosts do the I/O.
use std::collections::BTreeMap;

use kortex_ai::chat::{Message, Request, ToolCall};
use kortex_ai::consent::{Attachment, Kind, extract_quoted};
use kortex_ai::plan::{Context, Executed, PlannedCall, plan_call, summarize, to_workflow};
use kortex_ai::prompt;
use kortex_ai::stream::Turn;
use kortex_contract::ToolContract;
use kortex_runtime::ValueMap;
use kortex_workflow::{RunError, Workflow};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// Tool rounds the model may chain for one user message.
pub const MAX_ROUNDS: usize = 8;
/// Consecutive invalid tool turns corrected automatically before giving up.
pub const MAX_CORRECTIONS: usize = 2;
/// Gateway limits: messages kept in a request, and their total size.
const MAX_HISTORY_MESSAGES: usize = 50;
const MAX_HISTORY_CHARS: usize = 36_000;
/// Longest text kept in a saved conversation (characters).
const MAX_SAVED_TEXT_CHARS: usize = 20_000;

/// What the host does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// Call the model again (with the current history).
    Ask,
    /// The model answered in text: the turn is over.
    Answer,
    /// A plan waits for the user's decision.
    AwaitApproval,
    /// The loop ended early.
    Stop(Stop),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stop {
    TooManyRounds,
    TooManyCorrections,
}

/// One planned call as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanLine {
    pub call_id: String,
    pub tool: String,
    /// `texte → text`, `c1.text → text`…
    pub inputs: Vec<String>,
    pub params: Vec<(String, String)>,
    pub confirmations: Vec<String>,
}

/// What the interface shows, in order. Contents of results live in memory only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entry {
    User(String),
    Assistant(String),
    Plan(Vec<PlanLine>),
    Result {
        call_id: String,
        tool: String,
    },
    Failed {
        call_id: String,
        tool: String,
        code: String,
        detail: Option<String>,
    },
    Refused,
    Stopped(Stop),
}

/// Approval refused: a call needs an extra confirmation the user has not given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingConfirmation {
    pub call_id: String,
    pub kind: String,
}

/// The part of a conversation that survives a reload (no file contents, no results).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub lang: String,
    pub history: Vec<Message>,
    pub transcript: Vec<Entry>,
    pub texts: Vec<(usize, String, bool)>,
    pub executed: Vec<Executed>,
    pub next_call: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Conversation {
    lang: String,
    history: Vec<Message>,
    transcript: Vec<Entry>,
    attachments: Vec<Attachment>,
    results: BTreeMap<String, ValueMap>,
    executed: Vec<Executed>,
    pending: Option<Vec<PlannedCall>>,
    next_call: usize,
    rounds: usize,
    corrections: usize,
}

fn tool_json(code: &str, detail: Option<&str>) -> String {
    serde_json::json!({"error": code, "detail": detail}).to_string()
}

fn message_chars(message: &Message) -> usize {
    match message {
        Message::System(t) | Message::User(t) | Message::Tool { content: t, .. } => t.len(),
        Message::Assistant {
            content,
            tool_calls,
        } => {
            content.as_deref().map_or(0, str::len)
                + tool_calls
                    .iter()
                    .map(|c| c.name.len() + c.arguments.len() + 24)
                    .sum::<usize>()
        }
    }
}

impl Conversation {
    #[must_use]
    pub fn new(lang: &str) -> Self {
        Self {
            lang: lang.to_owned(),
            ..Self::default()
        }
    }

    pub fn set_lang(&mut self, lang: &str) {
        lang.clone_into(&mut self.lang);
    }

    #[must_use]
    pub fn transcript(&self) -> &[Entry] {
        &self.transcript
    }

    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    #[must_use]
    pub fn pending(&self) -> Option<&[PlannedCall]> {
        self.pending.as_deref()
    }

    /// Tools already run in this conversation: they stay offered so the model can chain on them.
    #[must_use]
    pub fn used_tools(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for call in &self.executed {
            if !ids.contains(&call.tool) {
                ids.push(call.tool.clone());
            }
        }
        ids
    }

    /// Output of a finished call, for display (never persisted).
    #[must_use]
    pub fn result(&self, call_id: &str) -> Option<&ValueMap> {
        self.results.get(call_id)
    }

    fn next_n(&self, kind: &Kind) -> usize {
        self.attachments
            .iter()
            .filter(|a| &a.kind == kind)
            .map(|a| a.n)
            .max()
            .unwrap_or(0)
            + 1
    }

    /// Adds a text the user typed or pasted; returns its number (`@texteN`).
    pub fn attach_text(&mut self, text: &str) -> usize {
        let n = self.next_n(&Kind::Text);
        self.attachments.push(Attachment::text(n, text));
        n
    }

    /// Adds a file (kept in memory only); returns its number (`@fichierN`).
    pub fn attach_file(&mut self, name: &str, mime: &str, bytes: Vec<u8>) -> usize {
        let n = self.next_n(&Kind::File);
        self.attachments.push(Attachment {
            n,
            kind: Kind::File,
            name: name.to_owned(),
            mime: mime.to_owned(),
            bytes,
            share_content: false,
        });
        n
    }

    /// The user's explicit choice to let the model read this attachment's content.
    pub fn set_share(&mut self, kind: &Kind, n: usize, share: bool) {
        if let Some(a) = self
            .attachments
            .iter_mut()
            .find(|a| a.n == n && &a.kind == kind)
        {
            a.share_content = share;
        }
    }

    pub fn remove_attachment(&mut self, kind: &Kind, n: usize) {
        self.attachments.retain(|a| !(a.n == n && &a.kind == kind));
    }

    /// A new user message: quoted passages become `@texteN` attachments, so their exact text
    /// stays local. Returns `false` for an empty message.
    pub fn submit(&mut self, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        let existing = self
            .attachments
            .iter()
            .filter(|a| a.kind == Kind::Text)
            .count();
        let (message, quoted) = extract_quoted(text.trim(), existing);
        for value in &quoted {
            self.attach_text(value);
        }
        self.transcript.push(Entry::User(text.trim().to_owned()));
        self.history.push(Message::User(message));
        self.rounds = 0;
        self.corrections = 0;
        true
    }

    /// The request for the model: the system prompt, then the most recent complete exchanges
    /// that fit the gateway's limits.
    #[must_use]
    pub fn request(&self, tools: Vec<Json>, stream: bool) -> Request {
        let mut start = 0;
        let fits = |from: usize| {
            self.history.len() - from <= MAX_HISTORY_MESSAGES
                && self.history[from..]
                    .iter()
                    .map(message_chars)
                    .sum::<usize>()
                    <= MAX_HISTORY_CHARS
        };
        // Drop whole exchanges from the front (each starts at a user message), never the last one.
        while !fits(start) {
            let next = (start + 1..self.history.len())
                .find(|&i| matches!(self.history[i], Message::User(_)));
            match next {
                Some(i) => start = i,
                None => break,
            }
        }
        let mut messages = vec![Message::System(prompt::system(
            &self.lang,
            &self.attachments,
        ))];
        messages.extend(self.history[start..].iter().cloned());
        Request {
            messages,
            tools,
            stream,
            max_tokens: 700,
        }
    }

    fn context<'a>(&'a self, contracts: &'a BTreeMap<String, ToolContract>) -> Context<'a> {
        Context {
            attachments: &self.attachments,
            results: &self.results,
            contracts,
        }
    }

    /// The model's turn arrived. Tool calls are renamed `c1`, `c2`… (short ids the model cannot
    /// garble), validated, and either corrected through the model or held as a plan.
    pub fn receive(&mut self, turn: Turn, contracts: &BTreeMap<String, ToolContract>) -> Next {
        if turn.tool_calls.is_empty() {
            let text = turn.content.unwrap_or_default();
            self.history.push(Message::Assistant {
                content: Some(if text.is_empty() {
                    "…".to_owned()
                } else {
                    text.clone()
                }),
                tool_calls: vec![],
            });
            if !text.is_empty() {
                self.transcript.push(Entry::Assistant(text));
            }
            return Next::Answer;
        }
        let calls: Vec<ToolCall> = turn
            .tool_calls
            .into_iter()
            .map(|c| {
                self.next_call += 1;
                ToolCall {
                    id: format!("c{}", self.next_call),
                    name: c.name,
                    arguments: c.arguments,
                }
            })
            .collect();
        self.history.push(Message::Assistant {
            content: turn.content.clone(),
            tool_calls: calls.clone(),
        });
        if let Some(text) = turn.content.filter(|t| !t.is_empty()) {
            self.transcript.push(Entry::Assistant(text));
        }
        self.rounds += 1;
        if self.rounds > MAX_ROUNDS {
            for call in &calls {
                self.history.push(Message::Tool {
                    call_id: call.id.clone(),
                    content: tool_json("too_many_steps", None),
                });
            }
            self.transcript.push(Entry::Stopped(Stop::TooManyRounds));
            return Next::Stop(Stop::TooManyRounds);
        }
        let planned: Vec<Result<PlannedCall, _>> = {
            let ctx = self.context(contracts);
            calls.iter().map(|c| plan_call(c, &ctx)).collect()
        };
        if planned.iter().any(Result::is_err) {
            let valid = self.context(contracts).valid_references();
            for (call, outcome) in calls.iter().zip(&planned) {
                let content = match outcome {
                    Err(error) => error.feedback(&valid),
                    Ok(_) => "Non exécuté : un autre appel de ce tour est invalide. Refais les appels corrigés.".to_owned(),
                };
                self.history.push(Message::Tool {
                    call_id: call.id.clone(),
                    content,
                });
            }
            self.corrections += 1;
            if self.corrections > MAX_CORRECTIONS {
                self.transcript
                    .push(Entry::Stopped(Stop::TooManyCorrections));
                return Next::Stop(Stop::TooManyCorrections);
            }
            return Next::Ask;
        }
        self.corrections = 0;
        let plan: Vec<PlannedCall> = planned.into_iter().flatten().collect();
        self.transcript
            .push(Entry::Plan(plan.iter().map(plan_line).collect()));
        self.pending = Some(plan);
        Next::AwaitApproval
    }

    /// The user approved the plan. `confirmed` holds `"c1:network"`-style keys for the extra
    /// confirmations (sensitive, network, write) the plan requires; all must be present.
    ///
    /// # Errors
    /// [`MissingConfirmation`] naming the first confirmation not given; nothing is approved.
    pub fn approve(
        &mut self,
        confirmed: &[String],
    ) -> Result<Vec<PlannedCall>, MissingConfirmation> {
        let Some(plan) = self.pending.as_ref() else {
            return Ok(Vec::new());
        };
        for call in plan {
            if let Some(kind) = call
                .confirmations
                .iter()
                .find(|k| !confirmed.contains(&format!("{}:{k}", call.call_id)))
            {
                return Err(MissingConfirmation {
                    call_id: call.call_id.clone(),
                    kind: (*kind).to_owned(),
                });
            }
        }
        Ok(plan.clone())
    }

    /// The outcome of one approved call; its summary (references, never data) goes to the model.
    pub fn record_result(&mut self, call_id: &str, result: Result<ValueMap, RunError>) {
        let Some(call) = self
            .pending
            .as_ref()
            .and_then(|p| p.iter().find(|c| c.call_id == call_id))
            .cloned()
        else {
            return;
        };
        match result {
            Ok(outputs) => {
                self.history.push(Message::Tool {
                    call_id: call_id.to_owned(),
                    content: summarize(call_id, &outputs),
                });
                self.transcript.push(Entry::Result {
                    call_id: call_id.to_owned(),
                    tool: call.tool.clone(),
                });
                self.executed.push(Executed {
                    call_id: call_id.to_owned(),
                    tool: call.tool,
                    params: call.params,
                    refs: call.refs,
                });
                self.results.insert(call_id.to_owned(), outputs);
            }
            Err(error) => {
                self.history.push(Message::Tool {
                    call_id: call_id.to_owned(),
                    content: tool_json(&error.code, error.detail.as_deref()),
                });
                self.transcript.push(Entry::Failed {
                    call_id: call_id.to_owned(),
                    tool: call.tool,
                    code: error.code,
                    detail: error.detail,
                });
            }
        }
    }

    /// Every approved call has reported: the model decides what comes next.
    pub fn finish_round(&mut self) -> Next {
        self.pending = None;
        Next::Ask
    }

    /// The user refused the plan: the model is told, and answers.
    pub fn refuse(&mut self) -> Next {
        for call in self.pending.take().unwrap_or_default() {
            self.history.push(Message::Tool {
                call_id: call.call_id,
                content: tool_json("refused_by_user", None),
            });
        }
        self.transcript.push(Entry::Refused);
        Next::Ask
    }

    /// Everything run so far as a Flow workflow.
    /// File attachments become file nodes the user fills again in Flow.
    #[must_use]
    pub fn export(&self, name: &str) -> Workflow {
        to_workflow(name, &self.executed, &self.attachments)
    }

    #[must_use]
    pub fn saved(&self) -> Saved {
        Saved {
            lang: self.lang.clone(),
            history: self.history.clone(),
            transcript: self.transcript.clone(),
            texts: self
                .attachments
                .iter()
                .filter(|a| a.kind == Kind::Text)
                .filter_map(|a| {
                    a.as_text()
                        .filter(|t| t.chars().count() <= MAX_SAVED_TEXT_CHARS)
                        .map(|t| (a.n, t.to_owned(), a.share_content))
                })
                .collect(),
            executed: self.executed.clone(),
            next_call: self.next_call,
        }
    }

    #[must_use]
    pub fn restore(saved: Saved) -> Self {
        let mut conversation = Self::new(&saved.lang);
        conversation.history = saved.history;
        conversation.transcript = saved.transcript;
        conversation.executed = saved.executed;
        conversation.next_call = saved.next_call;
        for (n, text, share) in saved.texts {
            conversation.attachments.push(Attachment {
                share_content: share,
                ..Attachment::text(n, &text)
            });
        }
        conversation
    }
}

/// Tools worth offering the model for a request, most relevant first. Offering every tool would cost
/// tokens and confuse the model, so Seek's search engine picks a handful; when the request matches
/// almost nothing the most popular tools pad the list so the model can still help.
#[must_use]
pub fn select_tools(
    contracts: &BTreeMap<String, ToolContract>,
    query: &str,
    lang: &str,
    limit: usize,
) -> Vec<String> {
    use kortex_search::{Doc, search};
    // References such as @texte1 are not words of the request.
    let words: String = query
        .split_whitespace()
        .filter(|w| !w.starts_with('@'))
        .collect::<Vec<_>>()
        .join(" ");
    let docs: Vec<Doc> = contracts
        .values()
        .map(|c| Doc {
            id: c.tool.id.clone(),
            variant: None,
            universe: c.tool.universe.clone(),
            names: c.tool.name.clone(),
            tool_names: BTreeMap::new(),
            keywords: c.search.keywords.clone(),
            intents: c.search.intents.clone(),
        })
        .collect();
    let mut ids: Vec<String> = search(&docs, &words, lang, limit)
        .into_iter()
        .map(|h| h.id)
        .collect();
    if ids.len() < 4 {
        let mut popular: Vec<&ToolContract> = contracts.values().collect();
        popular.sort_by(|a, b| {
            (a.tool.viral_priority, &a.tool.id).cmp(&(b.tool.viral_priority, &b.tool.id))
        });
        for contract in popular {
            if ids.len() >= 6 {
                break;
            }
            if !ids.contains(&contract.tool.id) {
                ids.push(contract.tool.id.clone());
            }
        }
    }
    ids
}

fn plan_line(call: &PlannedCall) -> PlanLine {
    use kortex_ai::plan::Reference;
    let name = |r: &Reference| match r {
        Reference::Text(n) => format!("texte{n}"),
        Reference::File(n) => format!("fichier{n}"),
        Reference::Result { call, port } => format!("{call}.{port}"),
    };
    PlanLine {
        call_id: call.call_id.clone(),
        tool: call.tool.clone(),
        inputs: call
            .refs
            .iter()
            .map(|(port, r)| format!("{} → {port}", name(r)))
            .collect(),
        params: call
            .params
            .iter()
            .map(|(k, v)| {
                let text = match v {
                    kortex_runtime::Value::Text(t) => t.clone(),
                    kortex_runtime::Value::Int(i) => i.to_string(),
                    kortex_runtime::Value::Number(n) => n.to_string(),
                    kortex_runtime::Value::Bool(b) => b.to_string(),
                    other => format!("{other:?}"),
                };
                (k.clone(), text)
            })
            .collect(),
        confirmations: call.confirmations.iter().map(|k| (*k).to_owned()).collect(),
    }
}
