use crate::config;
use crate::llm::Message;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created: String,
    pub updated: String,
    pub cwd: String,
    pub provider: String,
    pub model: String,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug)]
pub struct SessionMeta {
    pub id: String,
    pub updated: String,
    pub model: String,
    pub cwd: String,
    pub turns: usize,
}

fn dir() -> PathBuf {
    let d = config::dir().join("sessions");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn new_id() -> String {
    format!("{}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), std::process::id())
}

impl Session {
    pub fn new(cwd: &str, provider: &str, model: &str) -> Session {
        let now = chrono::Local::now().to_rfc3339();
        Session {
            id: new_id(),
            created: now.clone(),
            updated: now,
            cwd: cwd.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            messages: Vec::new(),
        }
    }

    pub fn save(&mut self) -> Result<()> {
        self.updated = chrono::Local::now().to_rfc3339();
        let path = dir().join(format!("{}.json", self.id));
        let body = serde_json::to_string_pretty(self)?;
        std::fs::write(path, body)?;
        Ok(())
    }

    pub fn load(id: &str) -> Result<Session> {
        let path = dir().join(format!("{id}.json"));
        let body = std::fs::read_to_string(&path).map_err(|_| anyhow!("session '{id}' not found"))?;
        Ok(serde_json::from_str(&body)?)
    }

    pub fn latest_for_cwd(cwd: &str) -> Option<Session> {
        let mut best: Option<Session> = None;
        if let Ok(rd) = std::fs::read_dir(dir()) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(body) = std::fs::read_to_string(&p) else { continue };
                let Ok(s) = serde_json::from_str::<Session>(&body) else { continue };
                if s.cwd != cwd {
                    continue;
                }
                if best.as_ref().map(|b| s.updated > b.updated).unwrap_or(true) {
                    best = Some(s);
                }
            }
        }
        best
    }

    pub fn list(limit: usize) -> Vec<SessionMeta> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(dir()) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(body) = std::fs::read_to_string(&p) else { continue };
                let Ok(s) = serde_json::from_str::<Session>(&body) else { continue };
                out.push(SessionMeta {
                    id: s.id,
                    updated: s.updated,
                    model: s.model,
                    cwd: s.cwd,
                    turns: s.messages.iter().filter(|m| m.role == crate::llm::Role::User).count(),
                });
            }
        }
        out.sort_by(|a, b| b.updated.cmp(&a.updated));
        out.truncate(limit);
        out
    }
}
