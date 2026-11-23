use super::*;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};

#[derive(Default)]
struct Acc {
    kind: String,
    id: String,
    name: String,
    json: String,
}

pub async fn complete(
    c: &Client,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Completion> {
    let url = endpoint(&c.base_url, "messages");

    let mut msgs: Vec<Value> = Vec::new();
    let mut i = 0usize;
    while i < messages.len() {
        match messages[i].role {
            Role::Tool => {
                let mut blocks: Vec<Value> = Vec::new();
                while i < messages.len() && messages[i].role == Role::Tool {
                    blocks.push(json!({
                        "type": "tool_result",
                        "tool_use_id": messages[i].tool_call_id.clone().unwrap_or_default(),
                        "content": messages[i].content
                    }));
                    i += 1;
                }
                msgs.push(json!({"role": "user", "content": blocks}));
            }
            Role::Assistant => {
                let m = &messages[i];
                let mut blocks: Vec<Value> = Vec::new();
                if !m.content.is_empty() {
                    blocks.push(json!({"type": "text", "text": m.content}));
                }
                for tc in &m.tool_calls {
                    let input: Value = serde_json::from_str(&tc.arguments).unwrap_or_else(|_| json!({}));
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": input
                    }));
                }
                if blocks.is_empty() {
                    blocks.push(json!({"type": "text", "text": "(empty)"}));
                }
                msgs.push(json!({"role": "assistant", "content": blocks}));
                i += 1;
            }
            _ => {
                msgs.push(json!({"role": "user", "content": messages[i].content}));
                i += 1;
            }
        }
    }

    let mut body = json!({
        "model": c.model,
        "max_tokens": c.max_tokens,
        "temperature": c.temperature,
        "system": system,
        "messages": msgs,
        "stream": c.stream
    });
    if !tools.is_empty() {
        let defs: Vec<Value> = tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "input_schema": t.parameters}))
            .collect();
        body["tools"] = json!(defs);
    }

    let mut req = c
        .http
        .post(&url)
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .json(&body);
    if !c.api_key.is_empty() {
        req = req.header("x-api-key", &c.api_key);
    }
    for (k, v) in &c.headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let resp = req.send().await.map_err(|e| anyhow!("request to {url} failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(anyhow!("{} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or(""), truncate(&txt, 900)));
    }

    if !c.stream {
        let v: Value = resp.json().await?;
        if let Some(err) = v.get("error") {
            if !err.is_null() {
                return Err(anyhow!("provider error: {}", truncate(&err.to_string(), 600)));
            }
        }
        let mut text = String::new();
        let mut calls = Vec::new();
        if let Some(blocks) = v["content"].as_array() {
            for b in blocks {
                match b["type"].as_str().unwrap_or("") {
                    "text" => text.push_str(b["text"].as_str().unwrap_or("")),
                    "tool_use" => calls.push(ToolCall {
                        id: b["id"].as_str().unwrap_or("").to_string(),
                        name: b["name"].as_str().unwrap_or("").to_string(),
                        arguments: b["input"].to_string(),
                    }),
                    _ => {}
                }
            }
        }
        if !text.is_empty() {
            on_text(&text);
        }
        return Ok(Completion {
            text,
            tool_calls: calls,
            finish: v["stop_reason"].as_str().unwrap_or("").to_string(),
            usage: Usage {
                input: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
                output: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
            },
        });
    }

    let mut stream = resp.bytes_stream();
    let mut lines = LineBuf::new();
    let mut sse = Sse::new();
    let mut text = String::new();
    let mut accs: Vec<Acc> = Vec::new();
    let mut usage = Usage::default();
    let mut finish = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("stream error: {e}"))?;
        for line in lines.push(&chunk) {
            let payload = match sse.feed(&line) {
                Some(p) => p,
                None => continue,
            };
            let v: Value = match serde_json::from_str(&payload) {
                Ok(v) => v,
                Err(_) => continue,
            };
            match v["type"].as_str().unwrap_or("") {
                "message_start" => {
                    usage.input = v["message"]["usage"]["input_tokens"].as_u64().unwrap_or(0);
                }
                "content_block_start" => {
                    let idx = v["index"].as_u64().unwrap_or(0) as usize;
                    while accs.len() <= idx {
                        accs.push(Acc::default());
                    }
                    let cb = &v["content_block"];
                    accs[idx].kind = cb["type"].as_str().unwrap_or("").to_string();
                    accs[idx].id = cb["id"].as_str().unwrap_or("").to_string();
                    accs[idx].name = cb["name"].as_str().unwrap_or("").to_string();
                }
                "content_block_delta" => {
                    let idx = v["index"].as_u64().unwrap_or(0) as usize;
                    while accs.len() <= idx {
                        accs.push(Acc::default());
                    }
                    let d = &v["delta"];
                    match d["type"].as_str().unwrap_or("") {
                        "text_delta" => {
                            if let Some(s) = d["text"].as_str() {
                                text.push_str(s);
                                on_text(s);
                            }
                        }
                        "input_json_delta" => {
                            if let Some(s) = d["partial_json"].as_str() {
                                accs[idx].json.push_str(s);
                            }
                        }
                        _ => {}
                    }
                }
                "message_delta" => {
                    if let Some(fr) = v["delta"]["stop_reason"].as_str() {
                        finish = fr.to_string();
                    }
                    if let Some(o) = v["usage"]["output_tokens"].as_u64() {
                        usage.output = o;
                    }
                }
                "error" => {
                    return Err(anyhow!("provider error: {}", truncate(&v["error"].to_string(), 600)));
                }
                _ => {}
            }
        }
    }

    let tool_calls: Vec<ToolCall> = accs
        .into_iter()
        .filter(|a| a.kind == "tool_use" && !a.name.is_empty())
        .enumerate()
        .map(|(i, a)| ToolCall {
            id: if a.id.is_empty() { format!("toolu_{}", i) } else { a.id },
            name: a.name,
            arguments: if a.json.trim().is_empty() { "{}".to_string() } else { a.json },
        })
        .collect();

    Ok(Completion { text, tool_calls, finish, usage })
}
