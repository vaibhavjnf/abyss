use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::cli::Args;
use crate::llm::{Client, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Yolo,
    Ask,
    Readonly,
}

impl Permission {
    pub fn parse(s: &str) -> Permission {
        match s.to_ascii_lowercase().as_str() {
            "ask" | "prompt" | "confirm" => Permission::Ask,
            "readonly" | "read-only" | "plan" | "ro" => Permission::Readonly,
            _ => Permission::Yolo,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Permission::Yolo => "yolo",
            Permission::Ask => "ask",
            Permission::Readonly => "readonly",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ProviderCfg {
    pub kind: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub permission: Option<String>,
    pub context_window: Option<u32>,
    pub max_turns: Option<u32>,
    pub stream: Option<bool>,
    pub system_extra: Option<String>,
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderCfg>,
}

fn merge_provider(a: ProviderCfg, b: ProviderCfg) -> ProviderCfg {
    ProviderCfg {
        kind: b.kind.or(a.kind),
        base_url: b.base_url.or(a.base_url),
        api_key: b.api_key.or(a.api_key),
        api_key_env: b.api_key_env.or(a.api_key_env),
        model: b.model.or(a.model),
        headers: {
            let mut map = a.headers;
            map.extend(b.headers);
            map
        },
    }
}

fn merge(a: Config, b: Config) -> Config {
    let mut providers = a.providers;
    for (k, v) in b.providers {
        let base = providers.remove(&k).unwrap_or_default();
        providers.insert(k, merge_provider(base, v));
    }
    Config {
        provider: b.provider.or(a.provider),
        model: b.model.or(a.model),
        base_url: b.base_url.or(a.base_url),
        api_key: b.api_key.or(a.api_key),
        api_key_env: b.api_key_env.or(a.api_key_env),
        max_tokens: b.max_tokens.or(a.max_tokens),
        temperature: b.temperature.or(a.temperature),
        permission: b.permission.or(a.permission),
        context_window: b.context_window.or(a.context_window),
        max_turns: b.max_turns.or(a.max_turns),
        stream: b.stream.or(a.stream),
        system_extra: b.system_extra.or(a.system_extra),
        providers,
    }
}

pub fn dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let d = home.join(".abyss");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn config_path() -> PathBuf {
    dir().join("config.toml")
}

fn load_file(path: &PathBuf) -> Config {
    match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

fn env_str(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

fn default_provider() -> String {
    if env_str("HINOKI_API_KEY").is_some() {
        "hinoki".to_string()
    } else if env_str("OPENAI_API_KEY").is_some() {
        "openai".to_string()
    } else {
        "hinoki".to_string()
    }
}

fn default_base(kind: Kind) -> &'static str {
    match kind {
        Kind::OpenAi => "https://api.openai.com/v1",
        Kind::Anthropic => "https://api.anthropic.com/v1",
        Kind::Gemini => "https://generativelanguage.googleapis.com/v1beta",
    }
}

fn default_key_env(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::OpenAi => Some("OPENAI_API_KEY"),
        Kind::Anthropic => Some("ANTHROPIC_API_KEY"),
        Kind::Gemini => Some("GEMINI_API_KEY"),
    }
}

pub fn preset(name: &str) -> Option<ProviderCfg> {
    let p = |kind: &str, base: &str, env: &str, model: &str| ProviderCfg {
        kind: Some(kind.to_string()),
        base_url: Some(base.to_string()),
        api_key: None,
        api_key_env: Some(env.to_string()),
        model: Some(model.to_string()),
        headers: BTreeMap::new(),
    };
    let cfg = match name {
        "hinoki" => p("openai", "http://127.0.0.1:8789/v1", "HINOKI_API_KEY", "hinoki-offensive"),
        "openai" => p("openai", "https://api.openai.com/v1", "OPENAI_API_KEY", "gpt-5"),
        "anthropic" => p("anthropic", "https://api.anthropic.com/v1", "ANTHROPIC_API_KEY", "claude-sonnet-4-5"),
        "openrouter" => p("openai", "https://openrouter.ai/api/v1", "OPENROUTER_API_KEY", "anthropic/claude-sonnet-4.5"),
        "groq" => p("openai", "https://api.groq.com/openai/v1", "GROQ_API_KEY", "llama-3.3-70b-versatile"),
        "ollama" => p("openai", "http://localhost:11434/v1", "OLLAMA_API_KEY", "qwen2.5-coder:7b"),
        "lmstudio" => p("openai", "http://localhost:1234/v1", "LMSTUDIO_API_KEY", "local-model"),
        "vllm" => p("openai", "http://localhost:8000/v1", "VLLM_API_KEY", "local-model"),
        "deepseek" => p("openai", "https://api.deepseek.com/v1", "DEEPSEEK_API_KEY", "deepseek-chat"),
        "xai" => p("openai", "https://api.x.ai/v1", "XAI_API_KEY", "grok-4"),
        "mistral" => p("openai", "https://api.mistral.ai/v1", "MISTRAL_API_KEY", "mistral-large-latest"),
        "together" => p("openai", "https://api.together.xyz/v1", "TOGETHER_API_KEY", "meta-llama/Llama-3.3-70B-Instruct-Turbo"),
        "cerebras" => p("openai", "https://api.cerebras.ai/v1", "CEREBRAS_API_KEY", "llama-3.3-70b"),
        "gemini" => p("gemini", "https://generativelanguage.googleapis.com/v1beta", "GEMINI_API_KEY", "gemini-2.5-pro"),
        _ => return None,
    };
    Some(cfg)
}

pub fn known_providers() -> Vec<&'static str> {
    vec![
        "hinoki", "openai", "anthropic", "openrouter", "groq", "ollama", "lmstudio", "vllm",
        "deepseek", "xai", "mistral", "together", "cerebras", "gemini",
    ]
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub provider: String,
    pub kind: Kind,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub permission: Permission,
    pub context_window: u32,
    pub max_turns: u32,
    pub headers: BTreeMap<String, String>,
    pub stream: bool,
    pub system_extra: Option<String>,
}

impl Resolved {
    pub fn client(&self) -> Client {
        Client::new(
            self.kind,
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
            self.max_tokens,
            self.temperature,
            self.headers.clone(),
            self.stream,
        )
    }

    pub fn key_status(&self) -> &'static str {
        if self.api_key.is_empty() {
            "none"
        } else {
            "set"
        }
    }

    pub fn switch_provider(&mut self, name: &str) -> Result<()> {
        let p = preset(name).ok_or_else(|| {
            anyhow!("unknown provider '{name}' — known: {}", known_providers().join(", "))
        })?;
        self.provider = name.to_string();
        self.kind = Kind::parse(p.kind.as_deref().unwrap_or("openai"))?;
        self.base_url = p.base_url.unwrap_or_else(|| default_base(self.kind).to_string());
        self.model = p.model.unwrap_or_default();
        self.api_key = p.api_key_env.as_deref().and_then(env_str).unwrap_or_default();
        self.headers = p.headers;
        Ok(())
    }
}

pub fn resolve(args: &Args) -> Result<Resolved> {
    let home = load_file(&config_path());
    let project = load_file(&PathBuf::from(".abyss.toml"));
    let cfg = merge(home, project);

    let provider = args
        .provider
        .clone()
        .or_else(|| env_str("ABYSS_PROVIDER"))
        .or_else(|| cfg.provider.clone())
        .unwrap_or_else(default_provider);

    let base_preset = preset(&provider).unwrap_or_default();
    let user_p = cfg.providers.get(&provider).cloned().unwrap_or_default();
    let pcfg = merge_provider(base_preset, user_p);

    let kind = Kind::parse(pcfg.kind.as_deref().unwrap_or("openai"))?;

    let base_url = args
        .base_url
        .clone()
        .or_else(|| env_str("ABYSS_BASE_URL"))
        .or_else(|| pcfg.base_url.clone())
        .unwrap_or_else(|| default_base(kind).to_string());

    let model = args
        .model
        .clone()
        .or_else(|| env_str("ABYSS_MODEL"))
        .or_else(|| cfg.model.clone())
        .or_else(|| pcfg.model.clone())
        .ok_or_else(|| anyhow!("no model configured — pass --model or set model in ~/.abyss/config.toml"))?;

    let api_key = args
        .api_key
        .clone()
        .or_else(|| env_str("ABYSS_API_KEY"))
        .or_else(|| pcfg.api_key.clone())
        .or_else(|| pcfg.api_key_env.as_deref().and_then(env_str))
        .or_else(|| cfg.api_key_env.as_deref().and_then(env_str))
        .or_else(|| default_key_env(kind).and_then(env_str))
        .unwrap_or_default();

    let permission = if args.yolo {
        Permission::Yolo
    } else if args.readonly {
        Permission::Readonly
    } else if args.ask {
        Permission::Ask
    } else {
        cfg.permission.as_deref().map(Permission::parse).unwrap_or(Permission::Yolo)
    };

    Ok(Resolved {
        provider,
        kind,
        base_url,
        model,
        api_key,
        max_tokens: cfg.max_tokens.unwrap_or(8192).max(256),
        temperature: cfg.temperature.unwrap_or(0.7),
        permission,
        context_window: cfg.context_window.unwrap_or(128_000),
        max_turns: cfg.max_turns.unwrap_or(args.max_turns).max(1),
        headers: pcfg.headers,
        stream: if args.no_stream { false } else { cfg.stream.unwrap_or(true) },
        system_extra: args.system_extra.clone().or(cfg.system_extra.clone()),
    })
}

const TEMPLATE: &str = r#"# ABYSS config — ~/.abyss/config.toml
# Everything is optional. Flags and env vars override this file.
#
# provider = "hinoki"          # hinoki | openai | anthropic | gemini | openrouter | groq | ollama | ...
# model = "hinoki-offensive"
# base_url = "http://127.0.0.1:8789/v1"
# permission = "yolo"          # yolo | ask | readonly
# max_tokens = 8192
# temperature = 0.7
# context_window = 128000
# stream = true

[providers.hinoki]
kind = "openai"
base_url = "http://127.0.0.1:8789/v1"
api_key_env = "HINOKI_API_KEY"
model = "hinoki-offensive"

[providers.openai]
kind = "openai"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
model = "gpt-5"

[providers.anthropic]
kind = "anthropic"
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-sonnet-4-5"

[providers.gemini]
kind = "gemini"
base_url = "https://generativelanguage.googleapis.com/v1beta"
api_key_env = "GEMINI_API_KEY"
model = "gemini-2.5-pro"

[providers.ollama]
kind = "openai"
base_url = "http://localhost:11434/v1"
api_key_env = "OLLAMA_API_KEY"
model = "qwen2.5-coder:7b"
"#;

pub fn write_default_config() -> (PathBuf, bool) {
    let path = config_path();
    if path.exists() {
        return (path, false);
    }
    let _ = std::fs::write(&path, TEMPLATE);
    (path, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_overrides_home() {
        let home = Config { model: Some("a".into()), temperature: Some(0.1), ..Default::default() };
        let proj = Config { model: Some("b".into()), ..Default::default() };
        let m = merge(home, proj);
        assert_eq!(m.model.as_deref(), Some("b"));
        assert_eq!(m.temperature, Some(0.1));
    }

    #[test]
    fn provider_merge_keeps_base_and_takes_model() {
        let a = ProviderCfg { base_url: Some("http://x".into()), ..Default::default() };
        let b = ProviderCfg { model: Some("m".into()), ..Default::default() };
        let m = merge_provider(a, b);
        assert_eq!(m.base_url.as_deref(), Some("http://x"));
        assert_eq!(m.model.as_deref(), Some("m"));
    }

    #[test]
    fn presets_cover_the_big_three() {
        assert_eq!(preset("anthropic").unwrap().kind.as_deref(), Some("anthropic"));
        assert_eq!(preset("gemini").unwrap().kind.as_deref(), Some("gemini"));
        assert_eq!(preset("hinoki").unwrap().kind.as_deref(), Some("openai"));
        assert!(preset("nope").is_none());
    }

    #[test]
    fn permission_parsing() {
        assert_eq!(Permission::parse("ask"), Permission::Ask);
        assert_eq!(Permission::parse("readonly"), Permission::Readonly);
        assert_eq!(Permission::parse("whatever"), Permission::Yolo);
    }
}
