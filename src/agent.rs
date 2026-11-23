use crate::config::Permission;
use crate::llm::{Client, Message, Usage};
use crate::tools::{self, TodoItem};
use crate::ui;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Agent {
    pub client: Arc<Client>,
    pub system: String,
    pub messages: Vec<Message>,
    pub max_turns: u32,
    pub permission: Permission,
    pub quiet: bool,
    pub usage: Usage,
    pub context_window: u32,
    pub cwd: PathBuf,
    pub depth: u32,
    pub todos: Arc<Mutex<Vec<TodoItem>>>,
}

impl Agent {
    pub fn new(
        client: Arc<Client>,
        system: String,
        permission: Permission,
        quiet: bool,
        max_turns: u32,
        cwd: PathBuf,
        context_window: u32,
    ) -> Agent {
        Agent {
            client,
            system,
            messages: Vec::new(),
            max_turns,
            permission,
            quiet,
            usage: Usage::default(),
            context_window,
            cwd,
            depth: 0,
            todos: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn env(&self) -> tools::Env {
        tools::Env {
            cwd: self.cwd.clone(),
            permission: self.permission,
            quiet: self.quiet,
            todos: self.todos.clone(),
            client: self.client.clone(),
            system: self.system.clone(),
            max_turns: self.max_turns,
            depth: self.depth,
        }
    }

    pub async fn run_turn(&mut self, input: &str) -> Result<String> {
        self.messages.push(Message::user(input));
        let final_text: String;
        let mut turn = 0u32;

        loop {
            turn += 1;
            if turn > self.max_turns {
                final_text = format!("[abyss] max_turns ({}) reached", self.max_turns);
                break;
            }
            self.maybe_compact().await?;

            let specs = tools::specs();
            let quiet = self.quiet;
            let mut spinner = if quiet { None } else { Some(ui::Spinner::start("thinking")) };
            let mut printed = false;

            let comp = {
                let mut cb = |s: &str| {
                    if let Some(sp) = spinner.take() {
                        sp.stop();
                    }
                    printed = true;
                    if !quiet {
                        ui::stream_delta(s);
                    }
                };
                self.client
                    .complete(&self.system, &self.messages, &specs, &mut cb)
                    .await?
            };

            if let Some(sp) = spinner.take() {
                sp.stop();
            }
            if printed && !quiet {
                ui::newline();
            }

            self.usage.add(&comp.usage);
            self.messages
                .push(Message::assistant(comp.text.clone(), comp.tool_calls.clone()));

            if comp.tool_calls.is_empty() {
                final_text = comp.text;
                break;
            }

            let env = self.env();
            for call in &comp.tool_calls {
                if !self.quiet {
                    ui::tool_call(&call.name, &call.arguments);
                }
                let args: Value = serde_json::from_str(&call.arguments).unwrap_or_else(|_| json!({}));
                let (out, is_err) = tools::execute(&call.name, &args, &env).await;
                if !self.quiet {
                    ui::tool_result(&out, is_err, false);
                }
                self.messages
                    .push(Message::tool(call.id.clone(), call.name.clone(), out));
            }
        }

        Ok(final_text)
    }

    fn est_tokens(&self) -> usize {
        let chars: usize = self
            .messages
            .iter()
            .map(|m| {
                m.content.chars().count()
                    + m.tool_calls.iter().map(|t| t.arguments.chars().count()).sum::<usize>()
            })
            .sum();
        chars / 4 + 1
    }

    async fn maybe_compact(&mut self) -> Result<()> {
        if self.messages.len() < 6 {
            return Ok(());
        }
        if self.est_tokens() < (self.context_window as usize * 3 / 4) {
            return Ok(());
        }
        self.compact().await
    }

    pub async fn compact(&mut self) -> Result<()> {
        if self.messages.len() < 4 {
            return Ok(());
        }
        let mut req = self.messages.clone();
        req.push(Message::user(
            "Summarize the conversation so far for your future self. Keep: the goal, decisions, files touched, commands that mattered, open threads. Be dense. No preamble.",
        ));
        let mut cb = |_: &str| {};
        let comp = self
            .client
            .complete(
                "You compress agent transcripts into dense handoff notes. No preamble.",
                &req,
                &[],
                &mut cb,
            )
            .await?;
        let summary = comp.text.trim().to_string();
        if summary.is_empty() {
            return Ok(());
        }
        self.usage.add(&comp.usage);
        let keep = 4.min(self.messages.len());
        let tail = self.messages.split_off(self.messages.len() - keep);
        self.messages.clear();
        self.messages
            .push(Message::user(format!("[compacted transcript — earlier context]\n{summary}")));
        self.messages.extend(tail);
        Ok(())
    }
}

pub async fn run_once(
    client: Arc<Client>,
    system: String,
    prompt: String,
    max_turns: u32,
    permission: Permission,
    cwd: PathBuf,
    depth: u32,
) -> Result<String> {
    let mut a = Agent::new(client, system, permission, true, max_turns, cwd, 128_000);
    a.depth = depth;
    a.run_turn(&prompt).await
}
