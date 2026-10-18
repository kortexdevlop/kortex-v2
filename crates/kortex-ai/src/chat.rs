//! Conversation messages and the gateway request body (`POST /v1/ai/chat`).
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A tool call proposed by the model. `name` is the function name (`text__cleaner`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON object text, exactly as the model produced it.
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    System(String),
    User(String),
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    /// The result of a tool call, as a short summary with references (never raw data).
    Tool {
        call_id: String,
        content: String,
    },
}

impl Message {
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::System(text) => json!({"role": "system", "content": text}),
            Self::User(text) => json!({"role": "user", "content": text}),
            Self::Assistant {
                content,
                tool_calls,
            } => {
                let mut message = json!({"role": "assistant", "content": content});
                if !tool_calls.is_empty() {
                    message["tool_calls"] = tool_calls
                        .iter()
                        .map(|c| {
                            json!({"id": c.id, "type": "function",
                                "function": {"name": c.name, "arguments": c.arguments}})
                        })
                        .collect();
                }
                message
            }
            Self::Tool { call_id, content } => {
                json!({"role": "tool", "tool_call_id": call_id, "content": content})
            }
        }
    }
}

/// One request to the gateway. `consent` must be true: the user agreed to send this content.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub messages: Vec<Message>,
    /// OpenAI-style tool objects, see [`crate::tools::definition`].
    pub tools: Vec<Value>,
    pub stream: bool,
    pub max_tokens: u32,
}

impl Request {
    #[must_use]
    pub fn body(&self) -> String {
        let mut body = json!({
            "consent": true,
            "messages": self.messages.iter().map(Message::to_json).collect::<Vec<_>>(),
            "max_tokens": self.max_tokens,
            "stream": self.stream,
        });
        if !self.tools.is_empty() {
            body["tools"] = Value::Array(self.tools.clone());
            body["tool_choice"] = json!("auto");
        }
        body.to_string()
    }
}

/// An error answer of the gateway (`{error, credits?}` with an HTTP status).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
}

impl ApiError {
    #[must_use]
    pub fn parse(status: u16, body: &str) -> Self {
        let code = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| "ai_unavailable".to_owned());
        Self { status, code }
    }
}
