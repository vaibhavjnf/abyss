pub mod anthropic;
pub mod gemini;
pub mod openai;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Message {
        Message { role: Role::System, content: content.into(), tool_calls: vec![], tool_call_id: None, name: None }
    }
    pub fn user(content: impl Into<String>) -> Message {
        Message { role: Role::User, content: content.into(), tool_calls: vec![], tool_call_id: None, name: None }
    }
    pub fn assistant(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Message {
        Message { role: Role::Assistant, content: content.into(), tool_calls, tool_call_id: None, name: None }
    }
    pub fn tool(id: impl Into<String>, name: impl Into<String>, content: impl Into<String>) -> Message {
        Message { role: Role::Tool, content: content.into(), tool_calls: vec![], tool_call_id: Some(id.into()), name: Some(name.into()) }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
}

impl Usage {
    pub fn add(&mut self, other: &Usage) {
        self.input += other.input;
        self.output += other.output;
    }
    pub fn total(&self) -> u64 {
        self.input + self.output
    }
}

#[derive(Clone, Debug, Default)]
pub struct Completion {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub finish: String,
    pub usage: Usage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    OpenAi,
    Anthropic,
    Gemini,
}

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        match s.to_ascii_lowercase().as_str() {
            "openai" | "openai-compatible" | "compat" | "oai" => Ok(Kind::OpenAi),
            "anthropic" | "claude" => Ok(Kind::Anthropic),
            "gemini" | "google" => Ok(Kind::Gemini),
            other => Err(anyhow!("unknown provider kind '{other}' (use openai | anthropic | gemini)")),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Kind::OpenAi => "openai-compatible",
            Kind::Anthropic => "anthropic",
            Kind::Gemini => "gemini",
        }
    }
}

pub struct Client {
    pub kind: Kind,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub headers: BTreeMap<String, String>,
    pub stream: bool,
    pub http: reqwest::Client,
}

impl Client {
    pub fn new(
        kind: Kind,
        base_url: String,
        api_key: String,
        model: String,
        max_tokens: u32,
        temperature: f32,
        headers: BTreeMap<String, String>,
        stream: bool,
    ) -> Client {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(900))
            .user_agent(concat!("abyss/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Client { kind, base_url, api_key, model, max_tokens, temperature, headers, stream, http }
    }

    pub async fn complete(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[ToolSpec],
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<Completion> {
        let mut last: Option<anyhow::Error> = None;
        for attempt in 0..3 {
            let res = match self.kind {
                Kind::OpenAi => openai::complete(self, system, messages, tools, on_text).await,
                Kind::Anthropic => anthropic::complete(self, system, messages, tools, on_text).await,
                Kind::Gemini => gemini::complete(self, system, messages, tools, on_text).await,
            };
            match res {
                Ok(c) => return Ok(c),
                Err(e) => {
                    let retryable = is_retryable(&e);
                    last = Some(e);
                    if !retryable || attempt == 2 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(600 * (attempt as u64 + 1))).await;
                }
            }
        }
        Err(last.unwrap_or_else(|| anyhow!("request failed")))
    }
}

fn is_retryable(e: &anyhow::Error) -> bool {
    let s = format!("{e:#}").to_ascii_lowercase();
    ["timeout", "timed out", "connect", "connection", "429", "500", "502", "503", "504", "overloaded", "reset", "temporarily"]
        .iter()
        .any(|k| s.contains(k))
}

pub(crate) fn endpoint(base: &str, path: &str) -> String {
    let b = base.trim_end_matches('/');
    if b.ends_with(path) {
        b.to_string()
    } else {
        format!("{}/{}", b, path)
    }
}

pub(crate) fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push_str("…");
        out
    }
}

/// Splits a byte stream into lines without breaking UTF-8 across chunks.
pub(crate) struct LineBuf {
    buf: Vec<u8>,
}

impl LineBuf {
    pub fn new() -> LineBuf {
        LineBuf { buf: Vec::new() }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            let pos = match self.buf.iter().position(|b| *b == b'\n') {
                Some(p) => p,
                None => break,
            };
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            out.push(String::from_utf8_lossy(&line).to_string());
        }
        out
    }
}

/// Minimal server-sent-events assembler: joins `data:` lines per event.
pub(crate) struct Sse {
    data: Vec<String>,
}

impl Sse {
    pub fn new() -> Sse {
        Sse { data: Vec::new() }
    }

    pub fn feed(&mut self, line: &str) -> Option<String> {
        if line.is_empty() {
            if self.data.is_empty() {
                return None;
            }
            let payload = self.data.join("\n");
            self.data.clear();
            return Some(payload);
        }
        if let Some(rest) = line.strip_prefix("data:") {
            self.data.push(rest.strip_prefix(' ').unwrap_or(rest).to_string());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_joins_without_duplicating() {
        assert_eq!(endpoint("http://x/v1", "chat/completions"), "http://x/v1/chat/completions");
        assert_eq!(
            endpoint("http://x/v1/chat/completions", "chat/completions"),
            "http://x/v1/chat/completions"
        );
        assert_eq!(endpoint("http://x/v1/", "messages"), "http://x/v1/messages");
    }

    #[test]
    fn sse_joins_multiline_data() {
        let mut sse = Sse::new();
        assert_eq!(sse.feed("event: message"), None);
        assert_eq!(sse.feed("data: {\"a\":1}"), None);
        assert_eq!(sse.feed(""), Some("{\"a\":1}".to_string()));
        assert_eq!(sse.feed("data: [DONE]"), None);
        assert_eq!(sse.feed(""), Some("[DONE]".to_string()));
        assert_eq!(sse.feed(""), None);
    }

    #[test]
    fn linebuf_survives_split_utf8_and_crlf() {
        let mut lb = LineBuf::new();
        let bytes = "héllo\r\nworld\n".as_bytes();
        let (a, b) = bytes.split_at(2);
        assert!(lb.push(a).is_empty());
        let mut lines = lb.push(b);
        lines.extend(lb.push(b""));
        assert_eq!(lines, vec!["héllo".to_string(), "world".to_string()]);
    }

    #[test]
    fn role_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Role::Tool).unwrap(), "\"tool\"");
        assert_eq!(serde_json::to_string(&Role::Assistant).unwrap(), "\"assistant\"");
    }
}
