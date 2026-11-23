use crate::agent::Agent;
use crate::config::{self, Permission, Resolved};
use crate::llm::{Client, Message, Role};
use crate::prompt;
use crate::session::Session;
use crate::ui;
use anyhow::{anyhow, Result};
use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{IsTerminal, Read};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "abyss",
    version,
    about = "ABYSS — abliterated terminal agent harness",
    long_about = "A dark, zero-config, provider-agnostic coding agent for the terminal.\nRuns on Linux, macOS and Windows. Speaks OpenAI-compatible, Anthropic and Gemini APIs."
)]
pub struct Args {
    /// Prompt to run. With a prompt, runs once and exits (like `claude -p`).
    #[arg(value_name = "PROMPT", trailing_var_arg = true)]
    pub prompt: Vec<String>,

    /// Force non-interactive mode
    #[arg(short = 'p', long = "print")]
    pub print: bool,

    /// Machine-readable JSON output (implies quiet)
    #[arg(long)]
    pub json: bool,

    /// Model id (overrides config)
    #[arg(short = 'm', long)]
    pub model: Option<String>,

    /// Provider preset: hinoki, openai, anthropic, gemini, openrouter, groq, ollama, ...
    #[arg(long)]
    pub provider: Option<String>,

    /// API base URL (overrides provider preset)
    #[arg(long = "base-url")]
    pub base_url: Option<String>,

    /// API key (overrides env)
    #[arg(long = "api-key")]
    pub api_key: Option<String>,

    /// Auto-approve every tool call
    #[arg(short = 'y', long)]
    pub yolo: bool,

    /// Read-only mode: no writes or edits
    #[arg(long)]
    pub readonly: bool,

    /// Ask before bash/write/edit
    #[arg(long)]
    pub ask: bool,

    /// Max agent turns per request
    #[arg(long = "max-turns", default_value_t = 60)]
    pub max_turns: u32,

    /// Resume the latest session for this directory
    #[arg(short = 'c', long = "continue")]
    pub continue_: bool,

    /// Resume a session by id
    #[arg(long)]
    pub resume: Option<String>,

    /// List saved sessions
    #[arg(long = "list-sessions")]
    pub list_sessions: bool,

    /// Write a starter config to ~/.abyss/config.toml
    #[arg(long)]
    pub init: bool,

    /// Print resolved config and test connectivity
    #[arg(long)]
    pub doctor: bool,

    /// Disable ANSI colors
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Extra system instructions
    #[arg(long = "system")]
    pub system_extra: Option<String>,

    /// Suppress streaming/tool output (final answer only)
    #[arg(long)]
    pub quiet: bool,

    /// Disable SSE streaming (use plain JSON responses)
    #[arg(long = "no-stream")]
    pub no_stream: bool,

    /// Preload a skill playbook into the system prompt (repeatable)
    #[arg(long = "skill", value_name = "NAME", action = clap::ArgAction::Append)]
    pub skill: Vec<String>,

    /// List the built-in and on-disk skills
    #[arg(long = "list-skills")]
    pub list_skills: bool,
}

pub async fn run() -> Result<i32> {
    let args = Args::parse();
    ui::init(args.no_color);

    if args.init {
        let (p, created) = config::write_default_config();
        if created {
            ui::ok(&format!("wrote starter config → {}", p.display()));
        } else {
            ui::info(&format!("config already exists → {}", p.display()));
        }
        return Ok(0);
    }

    if args.list_skills {
        let all = crate::skills::all();
        println!("{} skills (builtin + ~/.abyss/skills + ./.abyss/skills)", all.len());
        for s in all {
            println!("  {:<16} {:<8} {}", s.name, s.source, s.description);
        }
        return Ok(0);
    }

    let cfg = config::resolve(&args)?;

    if args.list_sessions {
        let list = Session::list(30);
        if list.is_empty() {
            ui::info("no sessions yet");
        }
        for m in list {
            println!("{}  {}  {}  {} turns  {}", m.id, m.updated, m.model, m.turns, m.cwd);
        }
        return Ok(0);
    }

    let client = Arc::new(cfg.client());

    if args.doctor {
        return doctor(&cfg, client).await;
    }

    if let Some(p) = take_prompt(&args)? {
        return print_mode(&args, cfg, client, p).await;
    }
    if args.print || args.json {
        return Err(anyhow!("no prompt given — pass text or pipe stdin"));
    }
    repl(&args, cfg, client).await
}

fn take_prompt(args: &Args) -> Result<Option<String>> {
    let joined = args.prompt.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(Some(joined));
    }
    if !std::io::stdin().is_terminal() {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        let t = s.trim();
        if !t.is_empty() {
            return Ok(Some(t.to_string()));
        }
    }
    Ok(None)
}

async fn print_mode(args: &Args, cfg: Resolved, client: Arc<Client>, prompt_text: String) -> Result<i32> {
    let cwd = std::env::current_dir()?;
    let cwd_s = cwd.to_string_lossy().to_string();
    let preloaded = crate::skills::preload(&args.skill)?;
    let system = prompt::system_prompt(&cwd, cfg.system_extra.as_deref(), &preloaded);
    let quiet = args.json || args.quiet;
    let mut agent = Agent::new(
        client,
        system,
        cfg.permission,
        quiet,
        cfg.max_turns,
        cwd,
        cfg.context_window,
    );

    let mut session = if args.continue_ {
        Session::latest_for_cwd(&cwd_s)
    } else if let Some(id) = &args.resume {
        Some(Session::load(id)?)
    } else {
        None
    };
    if let Some(s) = &session {
        agent.messages = s.messages.clone();
    }

    let text = agent.run_turn(&prompt_text).await?;

    let mut session = session.take().unwrap_or_else(|| Session::new(&cwd_s, &cfg.provider, &cfg.model));
    session.messages = agent.messages.clone();
    session.model = cfg.model.clone();
    session.provider = cfg.provider.clone();
    session.save()?;

    if args.json {
        let out = serde_json::json!({
            "text": text,
            "model": cfg.model,
            "provider": cfg.provider,
            "session": session.id,
            "usage": {"input": agent.usage.input, "output": agent.usage.output},
            "turns": agent.messages.iter().filter(|m| m.role == Role::User).count()
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if quiet {
        println!("{}", text);
    }
    Ok(0)
}

async fn repl(args: &Args, mut cfg: Resolved, client: Arc<Client>) -> Result<i32> {
    let cwd = std::env::current_dir()?;
    let cwd_s = cwd.to_string_lossy().to_string();
    let preloaded = crate::skills::preload(&args.skill)?;
    let system = prompt::system_prompt(&cwd, cfg.system_extra.as_deref(), &preloaded);
    let mut agent = Agent::new(
        client,
        system,
        cfg.permission,
        false,
        cfg.max_turns,
        cwd,
        cfg.context_window,
    );

    let loaded = if args.continue_ {
        Session::latest_for_cwd(&cwd_s)
    } else if let Some(id) = &args.resume {
        Some(Session::load(id)?)
    } else {
        None
    };
    if let Some(s) = &loaded {
        agent.messages = s.messages.clone();
    }
    let mut session = loaded.unwrap_or_else(|| Session::new(&cwd_s, &cfg.provider, &cfg.model));

    ui::banner(
        env!("CARGO_PKG_VERSION"),
        &cfg.model,
        &cfg.provider,
        &cwd_s,
        cfg.permission.label(),
    );

    let mut rl = DefaultEditor::new().map_err(|e| anyhow!("readline: {e}"))?;
    let hist = config::dir().join("history.txt");
    let _ = rl.load_history(&hist);

    loop {
        match rl.readline(&ui::prompt()) {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line.as_str());

                if line.starts_with('/') {
                    match slash(&line, &mut agent, &mut cfg, &mut session).await {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(e) => ui::error(&format!("{e:#}")),
                    }
                    continue;
                }

                match agent.run_turn(&line).await {
                    Ok(_) => {}
                    Err(e) => ui::error(&format!("{e:#}")),
                }
                session.messages = agent.messages.clone();
                session.model = cfg.model.clone();
                session.provider = cfg.provider.clone();
                let _ = session.save();
            }
            Err(ReadlineError::Interrupted) => {
                println!();
                continue;
            }
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                ui::error(&format!("readline: {e}"));
                break;
            }
        }
    }

    let _ = rl.save_history(&hist);
    Ok(0)
}

async fn slash(line: &str, agent: &mut Agent, cfg: &mut Resolved, session: &mut Session) -> Result<bool> {
    if let Some(cmd) = line.strip_prefix("/!") {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            ui::warn("usage: /!<shell command>");
            return Ok(true);
        }
        let (out, err) = crate::tools::execute("bash", &serde_json::json!({"command": cmd}), &agent.env()).await;
        ui::tool_result(&out, err, false);
        return Ok(true);
    }

    let mut parts = line.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();

    match cmd {
        "/exit" | "/quit" | "/q" => return Ok(false),
        "/help" | "/?" => print_help(),
        "/clear" => {
            agent.messages.clear();
            ui::ok("context cleared");
        }
        "/compact" => {
            agent.compact().await?;
            ui::ok(&format!("compacted — {} messages left", agent.messages.len()));
        }
        "/cost" => ui::info(&format!(
            "tokens in {} / out {} / total {}",
            agent.usage.input,
            agent.usage.output,
            agent.usage.total()
        )),
        "/tools" => {
            for t in crate::tools::specs() {
                let d: String = t.description.chars().take(100).collect();
                println!("  {} {}", ui::cyan(&t.name), ui::dim(&d));
            }
        }
        "/config" => print_config(cfg),
        "/model" => {
            if rest.is_empty() {
                ui::info(&format!("model: {}", cfg.model));
            } else {
                cfg.model = rest.to_string();
                agent.client = Arc::new(cfg.client());
                ui::ok(&format!("model → {}", cfg.model));
            }
        }
        "/provider" => {
            if rest.is_empty() {
                ui::info(&format!("provider: {} ({})", cfg.provider, cfg.kind.label()));
            } else {
                cfg.switch_provider(rest)?;
                agent.client = Arc::new(cfg.client());
                ui::ok(&format!(
                    "provider → {}  model → {}  base → {}",
                    cfg.provider, cfg.model, cfg.base_url
                ));
            }
        }
        "/yolo" => {
            cfg.permission = Permission::Yolo;
            agent.permission = Permission::Yolo;
            ui::ok("permission → yolo");
        }
        "/ask" => {
            cfg.permission = Permission::Ask;
            agent.permission = Permission::Ask;
            ui::ok("permission → ask");
        }
        "/readonly" => {
            cfg.permission = Permission::Readonly;
            agent.permission = Permission::Readonly;
            ui::ok("permission → readonly");
        }
        "/sessions" => {
            let list = Session::list(20);
            if list.is_empty() {
                ui::info("no sessions yet");
            }
            for m in list {
                println!("  {}  {}  {}  {}", m.id, m.updated, m.model, m.cwd);
            }
        }
        "/new" => {
            session.id = crate::session::new_id();
            session.messages.clear();
            agent.messages.clear();
            ui::ok(&format!("new session {}", session.id));
        }
        "/init" => {
            let (p, created) = config::write_default_config();
            if created {
                ui::ok(&format!("wrote starter config → {}", p.display()));
            } else {
                ui::info(&format!("config already exists → {}", p.display()));
            }
        }
        "/system" => println!("{}", agent.system),
        "/skills" => {
            for sk in crate::skills::all() {
                println!(
                    "  {:<16} {:<8} {}",
                    ui::cyan(&sk.name),
                    ui::dim(&sk.source),
                    ui::dim(&sk.description)
                );
            }
        }
        "/skill" => {
            if rest.is_empty() {
                ui::warn("usage: /skill <name>");
            } else {
                match crate::skills::get(rest) {
                    Some(sk) => println!("{}", sk.body),
                    None => ui::warn(&format!("unknown skill '{rest}' — try /skills")),
                }
            }
        }
        _ => ui::warn(&format!("unknown command {cmd} — try /help")),
    }
    Ok(true)
}

fn print_help() {
    println!("  {}", ui::bold("commands"));
    for (c, d) in [
        ("/help", "this"),
        ("/model [id]", "show or switch model"),
        ("/provider [name]", "show or switch provider preset"),
        ("/yolo /ask /readonly", "permission mode"),
        ("/clear", "wipe context"),
        ("/compact", "summarize context now"),
        ("/cost", "token usage"),
        ("/tools", "list tools"),
        ("/config", "resolved config"),
        ("/sessions", "saved sessions"),
        ("/new", "start a fresh session"),
        ("/init", "write starter config"),
        ("/system", "print system prompt"),
        ("/skills", "list skill playbooks"),
        ("/skill <name>", "print a skill playbook"),
        ("/!<cmd>", "run a shell command"),
        ("/exit", "leave"),
    ] {
        println!("  {}  {}", ui::cyan(&format!("{:<20}", c)), ui::dim(d));
    }
}

fn print_config(cfg: &Resolved) {
    println!("  provider     {}", cfg.provider);
    println!("  kind         {}", cfg.kind.label());
    println!("  base_url     {}", cfg.base_url);
    println!("  model        {}", cfg.model);
    println!("  api_key      {}", cfg.key_status());
    println!("  max_tokens   {}", cfg.max_tokens);
    println!("  temperature  {}", cfg.temperature);
    println!("  permission   {}", cfg.permission.label());
    println!("  context      {}", cfg.context_window);
    println!("  stream       {}", cfg.stream);
    println!("  config       {}", config::config_path().display());
}

async fn doctor(cfg: &Resolved, client: Arc<Client>) -> Result<i32> {
    ui::info(&format!("provider   {} ({})", cfg.provider, cfg.kind.label()));
    ui::info(&format!("base_url   {}", cfg.base_url));
    ui::info(&format!("model      {}", cfg.model));
    ui::info(&format!("api_key    {}", cfg.key_status()));
    ui::info(&format!("permission {}", cfg.permission.label()));
    ui::info(&format!("config     {}", config::config_path().display()));

    let t0 = std::time::Instant::now();
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
        Ok(c) => {
            ui::ok(&format!(
                "reachable in {} ms — replied: {}",
                t0.elapsed().as_millis(),
                ui::one_line(&c.text, 60)
            ));
            Ok(0)
        }
        Err(e) => {
            ui::error(&format!("unreachable: {e:#}"));
            Ok(1)
        }
    }
}
