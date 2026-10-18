//! Server-sent events of the gateway: an incremental parser and an assembler that rebuilds the
//! assistant turn (text and tool calls) from deltas.
use std::collections::BTreeMap;

use serde_json::Value;

use crate::chat::ToolCall;
use crate::credits::Credits;

/// A fragment of a tool call: the first carries `id` and `name`, the next ones argument text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Delta {
        content: Option<String>,
        tool_calls: Vec<Fragment>,
    },
    Finish(String),
    Done(Option<Credits>),
    Error(String),
}

/// Feed network chunks as they arrive; complete events come out, partial ones are kept.
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn feed(&mut self, chunk: &str) -> Vec<Event> {
        // A CRLF can arrive split across two chunks: normalise the whole buffer each time.
        self.buffer.push_str(chunk);
        self.buffer = self.buffer.replace("\r\n", "\n");
        let mut events = Vec::new();
        while let Some(end) = self.buffer.find("\n\n") {
            let block: String = self.buffer.drain(..end + 2).collect();
            events.extend(parse_block(&block));
        }
        events
    }
}

fn parse_block(block: &str) -> Vec<Event> {
    let mut name = "message";
    let mut data = String::new();
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            name = rest.trim();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim_start());
        }
    }
    let Ok(value) = serde_json::from_str::<Value>(&data) else {
        return Vec::new();
    };
    match name {
        "done" => vec![Event::Done(
            value.get("credits").and_then(Credits::from_json),
        )],
        "error" => vec![Event::Error(
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("ai_unavailable")
                .to_owned(),
        )],
        _ => {
            let mut events = Vec::new();
            let delta = value.get("delta").unwrap_or(&Value::Null);
            let content = delta
                .get("content")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let fragments: Vec<Fragment> = delta
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|c| Fragment {
                    index: c
                        .get("index")
                        .and_then(Value::as_u64)
                        .map_or(0, |i| i as usize),
                    id: c.get("id").and_then(Value::as_str).map(str::to_owned),
                    name: c
                        .pointer("/function/name")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    arguments: c
                        .pointer("/function/arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                })
                .collect();
            if content.is_some() || !fragments.is_empty() {
                events.push(Event::Delta {
                    content,
                    tool_calls: fragments,
                });
            }
            if let Some(reason) = value.get("finish_reason").and_then(Value::as_str) {
                events.push(Event::Finish(reason.to_owned()));
            }
            events
        }
    }
}

/// The assistant's turn once complete.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Turn {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
}

#[derive(Debug, Default)]
struct Partial {
    id: String,
    name: String,
    arguments: String,
}

/// Accumulates deltas into a [`Turn`]; `text()` shows the answer so far for live display.
#[derive(Debug, Default)]
pub struct Assembler {
    content: String,
    calls: BTreeMap<usize, Partial>,
    finish: Option<String>,
}

impl Assembler {
    pub fn push(&mut self, event: &Event) {
        match event {
            Event::Delta {
                content,
                tool_calls,
            } => {
                if let Some(text) = content {
                    self.content.push_str(text);
                }
                for f in tool_calls {
                    let call = self.calls.entry(f.index).or_default();
                    if let Some(id) = &f.id {
                        call.id.clone_from(id);
                    }
                    if let Some(name) = &f.name {
                        call.name.clone_from(name);
                    }
                    call.arguments.push_str(&f.arguments);
                }
            }
            Event::Finish(reason) => self.finish = Some(reason.clone()),
            Event::Done(_) | Event::Error(_) => {}
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn finish(self) -> Turn {
        Turn {
            content: (!self.content.is_empty()).then_some(self.content),
            tool_calls: self
                .calls
                .into_values()
                .filter(|c| !c.name.is_empty())
                .map(|c| ToolCall {
                    id: c.id,
                    name: c.name,
                    arguments: c.arguments,
                })
                .collect(),
            finish_reason: self.finish.unwrap_or_else(|| "stop".to_owned()),
        }
    }
}

/// A non-streaming gateway answer (`{message, finish_reason, credits}`).
#[must_use]
pub fn turn_from_response(body: &str) -> Option<(Turn, Option<Credits>)> {
    let value: Value = serde_json::from_str(body).ok()?;
    let message = value.get("message")?;
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some(ToolCall {
                id: c.get("id")?.as_str()?.to_owned(),
                name: c.pointer("/function/name")?.as_str()?.to_owned(),
                arguments: c.pointer("/function/arguments")?.as_str()?.to_owned(),
            })
        })
        .collect();
    let turn = Turn {
        content: message
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
        tool_calls,
        finish_reason: value
            .get("finish_reason")
            .and_then(Value::as_str)
            .unwrap_or("stop")
            .to_owned(),
    };
    Some((turn, value.get("credits").and_then(Credits::from_json)))
}
