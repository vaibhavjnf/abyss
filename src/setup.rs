use crate::config::{self, Permission};
use crate::llm::{Client, Kind, Message};
use crate::ui;
use anyhow::Result;
use std::collections::BTreeMap;
use std::io::{BufRead, IsTerminal, Write};

struct Preset {
    name: &'static str,
    label: &'static str,
    kind: Kind,
    base: &'static str,
    model: &'static str,
    env: &'static str,
}

const PRESETS: &[Preset] = &[
    Preset {
        name: "hinoki",
        label: "Hinoki local endpoint",
        kind: Kind::OpenAi,
        base: "http://127.0.0.1:8789/v1",
        model: "hinoki-offensive",
        env: "HINOKI_API_KEY",
    },
    Preset {
        name: "openai",
        label: "OpenAI",
        kind: Kind::OpenAi,
        base: "https://api.openai.com/v1",
        model: "gpt-5",
        env: "OPENAI_API_KEY",
    },
    Preset {
        name: "anthropic",
        label: "Anthropic",
        kind: Kind::Anthropic,
        base: "https://api.anthropic.com/v1",
        model: "claude-sonnet-4-5",
        env: "ANTHROPIC_API_KEY",
    },
    Preset {
        name: "gemini",
        label: "Google Gemini",
        kind: Kind::Gemini,
        base: "https://generativelanguage.googleapis.com/v1beta",
        model: "gemini-2.5-pro",
        env: "GEMINI_API_KEY",
    },
    Preset {
        name: "openrouter",
        label: "OpenRouter",
        kind: Kind::OpenAi,
        base: "https://openrouter.ai/api/v1",
        model: "anthropic/claude-sonnet-4.5",
        env: "OPENROUTER_API_KEY",
    },
    Preset {
        name: "groq",
        label: "Groq",
        kind: Kind::OpenAi,
        base: "https://api.groq.com/openai/v1",
        model: "llama-3.3-70b-versatile",
        env: "GROQ_API_KEY",
    },
    Preset {
        name: "deepseek",
        label: "DeepSeek",
        kind: Kind::OpenAi,
        base: "https://api.deepseek.com/v1",
        model: "deepseek-chat",
        env: "DEEPSEEK_API_KEY",
    },
    Preset {
        name: "xai",
        label: "xAI",
        kind: Kind::OpenAi,
        base: "https://api.x.ai/v1",
        model: "grok-4",
        env: "XAI_API_KEY",
    },
    Preset {
        name: "ollama",
        label: "Ollama (local)",
        kind: Kind::OpenAi,
        base: "http://localhost:11434/v1",
        model: "qwen2.5-coder:7b",
        env: "",
    },
    Preset {
        name: "custom",
        label: "Custom OpenAI-compatible",
        kind: Kind::OpenAi,
        base: "http://localhost:8000/v1",
        model: "local-model",
        env: "ABYSS_API_KEY",
    },
];

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::OpenAi => "openai",
        Kind::Anthropic => "anthropic",
        Kind::Gemini => "gemini",
    }
}

fn ask(label: &str, default: &str) -> String {
    if default.is_empty() {
        print!("  {} {} ", ui::cyan("?"), label);
    } else {
        print!("  {} {} {} ", ui::cyan("?"), label, ui::dim(&format!("[{}]", default)));
    }
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return default.to_string();
    }
    let t = line.trim();
    if t.is_empty() {
        default.to_string()
    } else {
        t.to_string()
    }
}

fn ask_secret(label: &str) -> String {
    print!("  {} {} ", ui::cyan("?"), label);
    let _ = std::io::stdout().flush();
    match rpassword::read_password() {
        Ok(s) => s.trim().to_string(),
        Err(_) => ask(label, ""),
    }
}

fn pick_provider() -> Option<&'static Preset> {
    println!();
    for (i, p) in PRESETS.iter().enumerate() {
        let env_state = if p.env.is_empty() {
            "no key".to_string()
        } else if std::env::var(p.env).is_ok() {
            format!("{} set", p.env)
        } else {
            format!("{} unset", p.env)
        };
        println!(
            "  {:>2}) {:<10} {:<26} {:<46} {}",
            i + 1,
            p.name,
            p.label,
            p.base,
            ui::dim(&env_state)
        );
    }
    let ans = ask("provider (number or name, q to skip)", "1");
    if ans.eq_ignore_ascii_case("q") {
        return None;
    }
    if let Ok(n) = ans.parse::<usize>() {
        if n >= 1 && n <= PRESETS.len() {
            return Some(&PRESETS[n - 1]);
        }
    }
    PRESETS.iter().find(|p| p.name.eq_ignore_ascii_case(&ans))
}

fn render(
    p: &Preset,
    base: &str,
    model: &str,
    key: Option<&str>,
    key_env: Option<&str>,
    permission: &str,
) -> String {
    let mut s = String::new();
    s.push_str("# ABYSS config — written by `abyss --setup`\n");
    s.push_str("# Flags and env vars still override this file.\n\n");
    s.push_str(&format!("provider = \"{}\"\n", p.name));
    s.push_str(&format!("model = \"{}\"\n", model));
    s.push_str(&format!("base_url = \"{}\"\n", base));
    s.push_str(&format!("permission = \"{}\"\n", permission));
    s.push_str("max_tokens = 8192\ntemperature = 0.7\ncontext_window = 128000\nstream = true\n\n");
    s.push_str(&format!("[providers.{}]\n", p.name));
    s.push_str(&format!("kind = \"{}\"\n", kind_name(p.kind)));
    s.push_str(&format!("base_url = \"{}\"\n", base));
    if let Some(k) = key {
        s.push_str(&format!("api_key = \"{}\"\n", k.replace('\\', "\\\\").replace('"', "\\\"")));
    }
    if let Some(e) = key_env {
        s.push_str(&format!("api_key_env = \"{}\"\n", e));
    }
    s.push_str(&format!("model = \"{}\"\n", model));
    s
}

fn write_config(text: &str) -> Result<std::path::PathBuf> {
    let path = config::config_path();
    if path.exists() {
        let bak = path.with_extension("toml.bak");
        let _ = std::fs::copy(&path, &bak);
    }
    std::fs::write(&path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(path)
}

pub async fn run() -> Result<()> {
    println!();
    println!(
        "  {} {}",
        ui::bold("ABYSS setup"),
        ui::dim("— 30 seconds, writes ~/.abyss/config.toml")
    );
    let interactive = std::io::stdin().is_terminal();

    let mut preset = &PRESETS[0];
    if interactive {
        match pick_provider() {
            Some(p) => preset = p,
            None => {
                ui::info("setup skipped — using built-in defaults");
                return Ok(());
            }
        }
    }

    let base = if interactive {
        ask("base url", preset.base)
    } else {
        preset.base.to_string()
    };
    let model = if interactive {
        ask("model", preset.model)
    } else {
        preset.model.to_string()
    };

    let mut key: Option<String> = None;
    let mut key_env: Option<String> = None;
    if interactive {
        if !preset.env.is_empty() && std::env::var(preset.env).is_ok() {
            let ans = ask(&format!("use ${} from env? (y/n)", preset.env), "y");
            if ans.eq_ignore_ascii_case("y") {
                key_env = Some(preset.env.to_string());
            } else {
                let k = ask_secret("api key (blank = none)");
                if !k.is_empty() {
                    key = Some(k);
                }
            }
        } else if !preset.env.is_empty() {
            let k = ask_secret(&format!("api key for {} (blank = none)", preset.name));
            if !k.is_empty() {
                key = Some(k);
            } else {
                key_env = Some(preset.env.to_string());
            }
        } else {
            let k = ask_secret("api key (blank = none)");
            if !k.is_empty() {
                key = Some(k);
            }
        }
    } else {
        if !preset.env.is_empty() {
            key = std::env::var(preset.env).ok();
            if key.is_none() {
                key_env = Some(preset.env.to_string());
            }
        }
    }

    let perm = if interactive {
        ask("permission (yolo/ask/readonly)", "yolo")
    } else {
        "yolo".to_string()
    };
    let permission = Permission::parse(&perm).label().to_string();

    let client = Client::new(
        preset.kind,
        base.clone(),
        key.clone().unwrap_or_default(),
        model.clone(),
        64,
        0.2,
        BTreeMap::new(),
        false,
    );
    print!("  {} testing {} … ", ui::cyan("•"), preset.name);
    let _ = std::io::stdout().flush();
    let mut cb = |_: &str| {};
    match client
        .complete(
            "You are a health check. Reply with exactly: ok",
            &[Message::user("ping")],
            &[],
            &mut cb,
        )
        .await
    {
        Ok(c) => println!("{}", ui::green(&format!("ok ({})", ui::one_line(&c.text, 24)))),
        Err(e) => println!("{}", ui::yellow(&format!("failed: {e:#}"))),
    }

    let text = render(preset, &base, &model, key.as_deref(), key_env.as_deref(), &permission);
    let path = write_config(&text)?;
    println!();
    ui::ok(&format!("wrote {}", path.display()));
    if let Some(bak) = path.with_extension("toml.bak").exists().then(|| path.with_extension("toml.bak")) {
        ui::info(&format!("previous config backed up to {}", bak.display()));
    }
    println!(
        "  {} {}",
        ui::dim("next:"),
        ui::cyan("abyss --doctor") + " " + &ui::dim("then") + " " + &ui::cyan("abyss")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn rendered_config_roundtrips() {
        let text = render(
            &PRESETS[0],
            "http://127.0.0.1:8789/v1",
            "hinoki-offensive",
            None,
            Some("HINOKI_API_KEY"),
            "yolo",
        );
        let c: Config = toml::from_str(&text).expect("valid toml");
        assert_eq!(c.provider.as_deref(), Some("hinoki"));
        assert_eq!(c.model.as_deref(), Some("hinoki-offensive"));
        let p = c.providers.get("hinoki").expect("provider block");
        assert_eq!(p.kind.as_deref(), Some("openai"));
        assert_eq!(p.api_key_env.as_deref(), Some("HINOKI_API_KEY"));
    }

    #[test]
    fn rendered_config_escapes_key() {
        let text = render(&PRESETS[1], "https://api.openai.com/v1", "gpt-5", Some("sk-\"x\""), None, "ask");
        let c: Config = toml::from_str(&text).expect("valid toml");
        assert_eq!(c.providers["openai"].api_key.as_deref(), Some("sk-\"x\""));
    }
}
