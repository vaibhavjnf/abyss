use super::*;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

fn push_parts(contents: &mut Vec<Value>, role: &str, parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    if let Some(last) = contents.last_mut() {
        if last["role"].as_str() == Some(role) {
            if let Some(arr) = last["parts"].as_array_mut() {
                arr.extend(parts);
                return;
            }
        }
    }
    contents.push(json!({"role": role, "parts": parts}));
}

pub async fn complete(
    c: &Client,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<Completion> {
    let base = c.base_url.trim_end_matches('/');
    let mut url = format!("{}/models/{}:generateContent", base, c.model);
    if !c.api_key.is_empty() {
        url = format!("{}?key={}", url, c.api_key);
    }

    let mut contents: Vec<Value> = Vec::new();
    for m in messages {
        match m.role {
            Role::Assistant => {
                let mut parts: Vec<Value> = Vec::new();
                if !m.content.is_empty() {
                    parts.push(json!({"text": m.content}));
                }
                for tc in &m.tool_calls {
                    let args: Value = serde_json::from_str(&tc.arguments).unwrap_or_else(|_| json!({}));
                    parts.push(json!({"functionCall": {"name": tc.name, "args": args}}));
                }
                push_parts(&mut contents, "model", parts);
            }
            Role::Tool => {
                let name = m.name.clone().unwrap_or_default();
                push_parts(
                    &mut contents,
                    "user",
                    vec![json!({"functionResponse": {"name": name, "response": {"result": m.content}}})],
                );
            }
            _ => {
                push_parts(&mut contents, "user", vec![json!({"text": m.content})]);
            }
        }
    }

    let mut body = json!({
        "contents": contents,
        "systemInstruction": {"parts": [{"text": system}]},
        "generationConfig": {"temperature": c.temperature, "maxOutputTokens": c.max_tokens}
    });
    if !tools.is_empty() {
        let decls: Vec<Value> = tools
            .iter()
            .map(|t| json!({"name": t.name, "description": t.description, "parameters": t.parameters}))
            .collect();
        body["tools"] = json!([{"functionDeclarations": decls}]);
    }

    let mut req = c.http.post(&url).header("content-type", "application/json").json(&body);
    for (k, v) in &c.headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let resp = req.send().await.map_err(|e| anyhow!("request to gemini failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(anyhow!("{} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or(""), truncate(&txt, 900)));
    }

    let v: Value = resp.json().await?;
    if let Some(err) = v.get("error") {
        if !err.is_null() {
            return Err(anyhow!("provider error: {}", truncate(&err.to_string(), 600)));
        }
    }
    let cand = &v["candidates"][0];
    let mut text = String::new();
    let mut calls = Vec::new();
    if let Some(parts) = cand["content"]["parts"].as_array() {
        for (i, p) in parts.iter().enumerate() {
            if let Some(t) = p["text"].as_str() {
                text.push_str(t);
            }
            if let Some(fc) = p.get("functionCall") {
                calls.push(ToolCall {
                    id: format!("gemini_{}", i),
                    name: fc["name"].as_str().unwrap_or("").to_string(),
                    arguments: fc["args"].to_string(),
                });
            }
        }
    }
    if !text.is_empty() {
        on_text(&text);
    }
    Ok(Completion {
        text,
        tool_calls: calls,
        finish: cand["finishReason"].as_str().unwrap_or("").to_string(),
        usage: Usage {
            input: v["usageMetadata"]["promptTokenCount"].as_u64().unwrap_or(0),
            output: v["usageMetadata"]["candidatesTokenCount"].as_u64().unwrap_or(0),
        },
    })
}
