# ABYSS

[![ci](https://github.com/vaibhavjnf/abyss/actions/workflows/ci.yml/badge.svg)](https://github.com/vaibhavjnf/abyss/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/vaibhavjnf/abyss?display_name=tag)](https://github.com/vaibhavjnf/abyss/releases)

**The abliterated terminal agent harness.** One static binary. No leash, no morals, no config required.

ABYSS is a Claude-Code-style coding agent for the CLI, written in Rust. It runs on **Linux, macOS and Windows**, talks to **any** model through OpenAI-compatible, Anthropic or Gemini APIs, and drives a real tool loop: shell, files, search, web, todos, subagents.

```
  ▄▀█ █▄▄ █▄█ █▀ █▀
  █▀█ █▄█  █  ▄█ ▄█
```

## Why

- **Abliterated by default.** The system prompt is direct: no hedging, no moralizing, no permission theater. It acts.
- **Zero-config.** `abyss "fix the build"` works. It auto-detects Hinoki, OpenAI, Anthropic or Gemini from env vars.
- **Any model.** Hinoki (`hinoki-offensive`), OpenAI, Anthropic, Gemini, OpenRouter, Groq, DeepSeek, xAI, Mistral, Together, Cerebras, Ollama, LM Studio, vLLM — anything that speaks one of the three protocols.
- **One binary.** No Node, no Python, no runtime. ~5 MB stripped release build.
- **Real tools.** Not a chat wrapper: it reads, edits, greps, runs, fetches, and spawns subagents.

## Install

### Prebuilt

Grab the latest from [Releases](https://github.com/vaibhavjnf/abyss/releases):

| platform | asset |
|---|---|
| Linux x86_64 | `abyss-linux-x86_64` |
| macOS arm64 | `abyss-macos-arm64` |
| Windows x86_64 | `abyss-windows-x86_64.exe` |

```bash
chmod +x abyss-linux-x86_64 && mv abyss-linux-x86_64 ~/.local/bin/abyss
```

### From source

Requires Rust 1.80+ (build only; the binary has no runtime deps).

```bash
git clone https://github.com/vaibhavjnf/abyss && cd abyss
cargo build --release
install -m 755 target/release/abyss ~/.local/bin/abyss   # macOS/Linux
```

Windows (PowerShell):

```powershell
cargo build --release
copy target\release\abyss.exe "$env:USERPROFILE\.local\bin\abyss.exe"
```

Or: `cargo install --git https://github.com/vaibhavjnf/abyss`

## Quick start

```bash
# Hinoki (workspace endpoint, no key needed)
export HINOKI_API_KEY=...            # optional for localhost
abyss "list the files here and summarize the project"

# one-shot, machine readable
abyss -p --json "run the tests and report failures"

# another provider
abyss --provider anthropic -m claude-sonnet-4-5 "refactor src/lib.rs"

# local model
abyss --provider ollama -m qwen2.5-coder:7b "explain this repo"

# interactive
abyss
```

If no prompt is given and stdin is not a TTY, stdin is used:

```bash
git diff | abyss -p "write a commit message for this"
```

## Providers

`--provider <name>` picks a preset; `--base-url`, `--model`, `--api-key` override anything.

| preset | protocol | default base | key env |
|---|---|---|---|
| `hinoki` | OpenAI | `http://127.0.0.1:8789/v1` | `HINOKI_API_KEY` |
| `openai` | OpenAI | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| `anthropic` | Anthropic | `https://api.anthropic.com/v1` | `ANTHROPIC_API_KEY` |
| `gemini` | Gemini | `https://generativelanguage.googleapis.com/v1beta` | `GEMINI_API_KEY` |
| `openrouter` | OpenAI | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
| `groq` | OpenAI | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` |
| `deepseek` | OpenAI | `https://api.deepseek.com/v1` | `DEEPSEEK_API_KEY` |
| `xai` | OpenAI | `https://api.x.ai/v1` | `XAI_API_KEY` |
| `mistral` | OpenAI | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` |
| `together` | OpenAI | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` |
| `cerebras` | OpenAI | `https://api.cerebras.ai/v1` | `CEREBRAS_API_KEY` |
| `ollama` | OpenAI | `http://localhost:11434/v1` | — |
| `lmstudio` | OpenAI | `http://localhost:1234/v1` | — |
| `vllm` | OpenAI | `http://localhost:8000/v1` | — |

Anything else: `--base-url https://my-endpoint/v1 --provider openai` (or `anthropic` / `gemini` for those wire formats).

`abyss --doctor` prints the resolved config and pings the endpoint.

## Config

`~/.abyss/config.toml` (created by `abyss --init`), plus a project-local `.abyss.toml` that overrides it.

```toml
provider = "hinoki"
model = "hinoki-offensive"
base_url = "http://127.0.0.1:8789/v1"
permission = "yolo"        # yolo | ask | readonly
max_tokens = 8192
temperature = 0.7
context_window = 128000
stream = true

[providers.hinoki]
kind = "openai"
base_url = "http://127.0.0.1:8789/v1"
api_key_env = "HINOKI_API_KEY"
model = "hinoki-offensive"
```

Env overrides: `ABYSS_PROVIDER`, `ABYSS_MODEL`, `ABYSS_BASE_URL`, `ABYSS_API_KEY`, `ABYSS_PERMISSION`.

Project context is auto-loaded from `ABYSS.md`, `AGENTS.md` or `CLAUDE.md` in the working directory.

## Tools

| tool | what it does |
|---|---|
| `bash` | run any shell command (cmd/PowerShell on Windows, sh/bash on unix), timeout + kill |
| `read` | read files with line numbers, offset/limit |
| `write` | create/overwrite, makes parent dirs |
| `edit` | exact string replace, uniqueness-checked |
| `glob` | find files by glob, newest first |
| `grep` | regex search, respects `.gitignore` |
| `ls` | directory listing with sizes |
| `webfetch` | fetch URL, strip HTML to text |
| `todo_write` | persistent task list for multi-step work |
| `task` | spawn an isolated subagent (same tools, depth 3) |

## Permission modes

- `yolo` (default): every tool runs.
- `ask`: prompts `[Y/n]` before `bash`, `write`, `edit`.
- `readonly`: blocks `write` and `edit`.

Flags: `-y/--yolo`, `--ask`, `--readonly`.

## Slash commands (interactive)

```
/help /model /provider /yolo /ask /readonly /clear /compact /cost
/tools /config /sessions /new /init /system /!<cmd> /exit
```

`/!ls -la` runs a shell command directly. `Ctrl-C` cancels the line, `Ctrl-D` exits.

## Sessions

Every turn is persisted to `~/.abyss/sessions/<id>.json`.

```bash
abyss -c "keep going"          # resume latest session for this directory
abyss --resume 20261123-121400-1234 "and now the tests"
abyss --list-sessions
```

Auto-compaction kicks in at ~75% of `context_window`: the transcript is summarized into a dense handoff note and the last few messages are kept.

## CLI

```
abyss [PROMPT...] [OPTIONS]

-p, --print            non-interactive
    --json             JSON result (text, usage, session)
-m, --model <id>
    --provider <name>
    --base-url <url>
    --api-key <key>
-y, --yolo
    --ask
    --readonly
    --max-turns <n>    default 60
-c, --continue
    --resume <id>
    --list-sessions
    --init
    --doctor
    --system <text>
    --quiet
    --no-stream        plain JSON instead of SSE
    --no-color
```

## Cross-platform notes

- Shell: `sh -c` on unix, `cmd /C` on Windows; `shell: "powershell"` / `"pwsh"` / `"bash"` per call.
- Paths are `PathBuf` everywhere; no unix-only syscalls.
- ANSI is enabled on Windows consoles via `ENABLE_VIRTUAL_TERMINAL_PROCESSING`.
- TLS is rustls — no OpenSSL, no system cert headaches.

## Build

```bash
cargo build --release      # target/release/abyss
cargo test                 # 12 unit tests: SSE, line splitting, config merge, tools
```

Layout:

```
src/main.rs        entry
src/cli.rs         args, REPL, print mode, doctor
src/config.rs      presets, config merge, resolution
src/agent.rs       tool loop, compaction, subagents
src/llm/           openai.rs, anthropic.rs, gemini.rs + SSE
src/tools/         shell, fs, search, web, todo, task
src/session.rs     persistence
src/prompt.rs      the personality
src/ui.rs          colors, spinner, rendering
```

## License

MIT.
