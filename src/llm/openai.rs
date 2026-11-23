use super::*;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};

fn text_of(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(arr) = v.as_array() {
        let mut out = String::new();
        for part in arr {
            if let Some(s) = part["text"].as_str() {
                out.push_str(s);
            }
        }
        return out;
    }
    String::new()
}


async fn send(c: &Client, url: &str, body: &Value) -> Result<reqwest::Response> {
    let mut req = c.http.post(url).header("content-type", "application/json").json(body);
    if !c.api_key.is_empty() {
        req = req.bearer_auth(&c.api_key);
    }
    for (k, v) in &c.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    req.send().await.map_err(|e| anyhow!("request to {url} failed: {e}"))
}

pub async fn complete(
    c: &Client,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Completion> {
    let url = endpoint(&c.base_url, "chat/completions");

    let mut msgs: Vec<Value> = Vec::with_capacity(messages.len() + 1);
    msgs.push(json!({"role": "system", "content": system}));
    for m in messages {
        match m.role {
            Role::System => msgs.push(json!({"role": "system", "content": m.content})),
            Role::User => msgs.push(json!({"role": "user", "content": m.content})),
            Role::Assistant => {
                let mut v = json!({
                    "role": "assistant",
                    "content": if m.content.is_empty() { Value::Null } else { json!(m.content) }
                });
                if !m.tool_calls.is_empty() {
                    let calls: Vec<Value> = m
                        .tool_calls
                        .iter()
                        .map(|t| {
                            json!({
                                "id": t.id,
                                "type": "function",
                                "function": {"name": t.name, "arguments": t.arguments}
                            })
                        })
                        .collect();
                    v["tool_calls"] = json!(calls);
                }
                msgs.push(v);
            }
            Role::Tool => msgs.push(json!({
                "role": "tool",
                "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
                "content": m.content
            })),
        }
    }

    let mut body = json!({
        "model": c.model,
        "messages": msgs,
        "stream": c.stream,
        "temperature": c.temperature,
        "max_tokens": c.max_tokens
    });
    if !tools.is_empty() {
        let defs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {"name": t.name, "description": t.description, "parameters": t.parameters}
                })
            })
            .collect();
        body["tools"] = json!(defs);
        body["tool_choice"] = json!("auto");
    }
    if c.stream {
        body["stream_options"] = json!({"include_usage": true});
    }

    let mut resp = send(c, &url, &body).await?;
    if resp.status().as_u16() == 400 {
        let txt = resp.text().await.unwrap_or_default();
        let mut retry = false;
        if txt.contains("stream_options") {
            if let Some(obj) = body.as_object_mut() {
                obj.remove("stream_options");
            }
            retry = true;
        } else if txt.contains("max_tokens") {
            if let Some(obj) = body.as_object_mut() {
                let mt = obj.remove("max_tokens").unwrap_or(Value::Null);
                obj.insert("max_completion_tokens".to_string(), mt);
            }
            retry = true;
        }
        if !retry {
            return Err(anyhow!("400 Bad Request: {}", truncate(&txt, 900)));
        }
        resp = send(c, &url, &body).await?;
    }
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
        let choice = &v["choices"][0];
        let msg = &choice["message"];
        let text = text_of(&msg["content"]);
        let mut calls = Vec::new();
        if let Some(tcs) = msg["tool_calls"].as_array() {
            for tc in tcs {
                calls.push(ToolCall {
                    id: tc["id"].as_str().unwrap_or("").to_string(),
                    name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                    arguments: tc["function"]["arguments"].as_str().unwrap_or("{}").to_string(),
                });
            }
        }
        let usage = Usage {
            input: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            output: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
        };
        if !text.is_empty() {
            on_text(&text);
        }
        return Ok(Completion {
            text,
            tool_calls: calls,
            finish: choice["finish_reason"].as_str().unwrap_or("").to_string(),
            usage,
        });
    }

    let mut stream = resp.bytes_stream();
    let mut lines = LineBuf::new();
    let mut sse = Sse::new();
    let mut text = String::new();
    let mut calls: Vec<(String, String, String)> = Vec::new();
    let mut usage = Usage::default();
    let mut finish = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("stream error: {e}"))?;
        for line in lines.push(&chunk) {
            let payload = match sse.feed(&line) {
                Some(p) => p,
                None => continue,
            };
            if payload.trim() == "[DONE]" {
                continue;
            }
            let v: Value = match serde_json::from_str(&payload) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(err) = v.get("error") {
                if !err.is_null() {
                    return Err(anyhow!("provider error: {}", truncate(&err.to_string(), 600)));
                }
            }
            if let Some(u) = v.get("usage") {
                if !u.is_null() {
                    usage.input = u["prompt_tokens"].as_u64().unwrap_or(usage.input);
                    usage.output = u["completion_tokens"].as_u64().unwrap_or(usage.output);
                }
            }
            let choice = match v["choices"].get(0) {
                Some(ch) => ch,
                None => continue,
            };
            if let Some(delta) = choice.get("delta") {
                if let Some(s) = delta["content"].as_str() {
                    if !s.is_empty() {
                        text.push_str(s);
                        on_text(s);
                    }
                }
                if let Some(tcs) = delta["tool_calls"].as_array() {
                    for tc in tcs {
                        let idx = tc["index"].as_u64().unwrap_or(0) as usize;
                        while calls.len() <= idx {
                            calls.push((String::new(), String::new(), String::new()));
                        }
                        if let Some(id) = tc["id"].as_str() {
                            if !id.is_empty() {
                                calls[idx].0 = id.to_string();
                            }
                        }
                        if let Some(n) = tc["function"]["name"].as_str() {
                            if !n.is_empty() {
                                calls[idx].1.push_str(n);
                            }
                        }
                        if let Some(a) = tc["function"]["arguments"].as_str() {
                            calls[idx].2.push_str(a);
                        }
                    }
                }
            }
            if let Some(fr) = choice["finish_reason"].as_str() {
                if !fr.is_empty() {
                    finish = fr.to_string();
                }
            }
        }
    }

    let tool_calls: Vec<ToolCall> = calls
        .into_iter()
        .enumerate()
        .filter(|(_, (_, name, _))| !name.is_empty())
        .map(|(i, (id, name, args))| ToolCall {
            id: if id.is_empty() { format!("call_{}", i) } else { id },
            name,
            arguments: if args.trim().is_empty() { "{}".to_string() } else { args },
        })
        .collect();

    Ok(Completion { text, tool_calls, finish, usage })
}
