use crate::config::Permission;
use crate::llm::{Client, ToolSpec};
use globset::GlobBuilder;
use regex::RegexBuilder;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
}

pub struct Env {
    pub cwd: PathBuf,
    pub permission: Permission,
    pub quiet: bool,
    pub todos: Arc<Mutex<Vec<TodoItem>>>,
    pub client: Arc<Client>,
    pub system: String,
    pub max_turns: u32,
    pub depth: u32,
}

fn s(args: &Value, key: &str) -> String {
    args[key].as_str().unwrap_or("").to_string()
}

fn resolve(cwd: &Path, p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        cwd.join(pb)
    }
}

fn truncate_mid(s: &str, limit: usize) -> String {
    let n = s.chars().count();
    if n <= limit {
        return s.to_string();
    }
    let head: String = s.chars().take(limit * 6 / 10).collect();
    let tail_rev: String = s.chars().rev().take(limit * 3 / 10).collect();
    let tail: String = tail_rev.chars().rev().collect();
    format!("{}\n… [{} chars cut] …\n{}", head, n - limit, tail)
}

fn human_size(n: u64) -> String {
    if n < 1024 {
        format!("{}B", n)
    } else if n < 1024 * 1024 {
        format!("{:.1}K", n as f64 / 1024.0)
    } else {
        format!("{:.1}M", n as f64 / (1024.0 * 1024.0))
    }
}

fn confirm(what: &str) -> bool {
    print!("  {} {} {} ", crate::ui::yellow("?"), what, crate::ui::dim("[Y/n]"));
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return true;
    }
    let l = line.trim().to_ascii_lowercase();
    l.is_empty() || l == "y" || l == "yes"
}

fn gate(env: &Env, what: &str) -> bool {
    match env.permission {
        Permission::Yolo => true,
        Permission::Readonly => true,
        Permission::Ask => confirm(what),
    }
}

pub fn specs() -> Vec<ToolSpec> {
    let spec = |name: &str, description: &str, parameters: Value| ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    };
    vec![
        spec(
            "bash",
            "Run a shell command and return stdout+stderr. Use it for everything: builds, tests, git, curl, package managers, one-liners. Default timeout 120s.",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Command to execute"},
                    "timeout_secs": {"type": "integer", "description": "Kill after N seconds (default 120, max 3600)"},
                    "cwd": {"type": "string", "description": "Working directory (default: project root)"},
                    "shell": {"type": "string", "description": "unix: sh|bash|zsh; windows: cmd|powershell|pwsh"}
                },
                "required": ["command"]
            }),
        ),
        spec(
            "read",
            "Read a text file with line numbers. Use offset/limit for big files.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "offset": {"type": "integer", "description": "1-based first line"},
                    "limit": {"type": "integer", "description": "Max lines (default 2000)"}
                },
                "required": ["path"]
            }),
        ),
        spec(
            "write",
            "Create or overwrite a file. Parent directories are created.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
        ),
        spec(
            "edit",
            "Exact string replacement in a file. old_string must match verbatim; it must be unique unless replace_all is true.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"},
                    "replace_all": {"type": "boolean"}
                },
                "required": ["path", "old_string", "new_string"]
            }),
        ),
        spec(
            "glob",
            "Find files by glob pattern (e.g. '**/*.rs', 'src/**/*.toml'). Returns paths sorted by modification time.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string", "description": "Base directory (default: project root)"}
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "grep",
            "Search file contents with a regex. Returns path:line: text. Respects .gitignore.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string", "description": "Rust regex"},
                    "path": {"type": "string", "description": "Base directory or file"},
                    "glob": {"type": "string", "description": "Only search files matching this glob"},
                    "ignore_case": {"type": "boolean"},
                    "max_results": {"type": "integer", "description": "Default 200"}
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "ls",
            "List a directory (dirs first, then files with sizes).",
            json!({
                "type": "object",
                "properties": {"path": {"type": "string", "description": "Default: project root"}}
            }),
        ),
        spec(
            "webfetch",
            "Fetch a URL and return readable text (HTML is stripped).",
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "max_chars": {"type": "integer", "description": "Default 20000"}
                },
                "required": ["url"]
            }),
        ),
        spec(
            "todo_write",
            "Replace the session todo list. Use for any task with 3+ steps; keep exactly one item in_progress.",
            json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]}
                            },
                            "required": ["content", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        ),
        spec(
            "task",
            "Spawn an isolated subagent with the same tools to do one job. Returns its final answer. Use for parallel research or self-contained sub-problems.",
            json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "Complete, self-contained instructions"},
                    "max_turns": {"type": "integer", "description": "Default: parent limit"}
                },
                "required": ["prompt"]
            }),
        ),
    ]
}

pub async fn execute(name: &str, args: &Value, env: &Env) -> (String, bool) {
    match name {
        "bash" => shell(args, env).await,
        "read" => read_file(args, env).await,
        "write" => write_file(args, env).await,
        "edit" => edit_file(args, env).await,
        "glob" => glob_files(args, env),
        "grep" => grep_files(args, env),
        "ls" => list_dir(args, env),
        "webfetch" => webfetch(args).await,
        "todo_write" => todo_write(args, env),
        "task" => task(args, env).await,
        other => (format!("unknown tool '{other}'"), true),
    }
}

async fn shell(args: &Value, env: &Env) -> (String, bool) {
    let command = s(args, "command");
    if command.trim().is_empty() {
        return ("command is empty".to_string(), true);
    }
    if !gate(env, &format!("bash({})", crate::ui::one_line(&command, 80))) {
        return ("denied by operator".to_string(), true);
    }
    let timeout = args["timeout_secs"].as_u64().unwrap_or(120).clamp(1, 3600);
    let cwd = args["cwd"].as_str().map(|p| resolve(&env.cwd, p)).unwrap_or_else(|| env.cwd.clone());
    let shell = s(args, "shell");

    let mut cmd = if cfg!(windows) {
        if shell.eq_ignore_ascii_case("powershell") || shell.eq_ignore_ascii_case("pwsh") {
            let exe = if shell.eq_ignore_ascii_case("pwsh") { "pwsh" } else { "powershell" };
            let mut c = Command::new(exe);
            c.args(["-NoProfile", "-Command", &command]);
            c
        } else {
            let mut c = Command::new("cmd");
            c.args(["/C", &command]);
            c
        }
    } else {
        let sh = if shell.is_empty() { "sh" } else { shell.as_str() };
        let mut c = Command::new(sh);
        c.args(["-c", &command]);
        c
    };

    cmd.current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    match tokio::time::timeout(Duration::from_secs(timeout), cmd.output()).await {
        Err(_) => (format!("timed out after {}s", timeout), true),
        Ok(Err(e)) => (format!("failed to spawn: {e}"), true),
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let mut text = String::new();
            if !stdout.trim().is_empty() {
                text.push_str(&stdout);
            }
            if !stderr.trim().is_empty() {
                if !text.is_empty() {
                    text.push_str("\n[stderr]\n");
                }
                text.push_str(&stderr);
            }
            if text.trim().is_empty() {
                text.push_str("(no output)");
            }
            if !out.status.success() {
                text.push_str(&format!("\n[exit {}]", out.status.code().unwrap_or(-1)));
            }
            (truncate_mid(&text, 30_000), !out.status.success())
        }
    }
}

async fn read_file(args: &Value, env: &Env) -> (String, bool) {
    let path = resolve(&env.cwd, &s(args, "path"));
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) => return (format!("{}: {e}", path.display()), true),
    };
    let content = String::from_utf8_lossy(&bytes);
    let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = args["limit"].as_u64().unwrap_or(2000).max(1) as usize;
    let total = content.lines().count();
    let mut out = String::new();
    for (i, line) in content.lines().enumerate().skip(offset - 1).take(limit) {
        out.push_str(&format!("{:>6}\t{}\n", i + 1, line));
    }
    if out.is_empty() {
        out.push_str("(empty)");
    } else if offset - 1 + limit < total {
        out.push_str(&format!("… ({} of {} lines shown)\n", limit, total));
    }
    (out, false)
}

async fn write_file(args: &Value, env: &Env) -> (String, bool) {
    let path = resolve(&env.cwd, &s(args, "path"));
    if env.permission == Permission::Readonly {
        return ("readonly mode: write blocked".to_string(), true);
    }
    if !gate(env, &format!("write({})", path.display())) {
        return ("denied by operator".to_string(), true);
    }
    let content = s(args, "content");
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return (format!("mkdir {}: {e}", parent.display()), true);
            }
        }
    }
    match tokio::fs::write(&path, content.as_bytes()).await {
        Ok(_) => (format!("wrote {} bytes → {}", content.len(), path.display()), false),
        Err(e) => (format!("{}: {e}", path.display()), true),
    }
}

async fn edit_file(args: &Value, env: &Env) -> (String, bool) {
    let path = resolve(&env.cwd, &s(args, "path"));
    if env.permission == Permission::Readonly {
        return ("readonly mode: edit blocked".to_string(), true);
    }
    if !gate(env, &format!("edit({})", path.display())) {
        return ("denied by operator".to_string(), true);
    }
    let old = s(args, "old_string");
    let new = s(args, "new_string");
    if old.is_empty() {
        return ("old_string is empty".to_string(), true);
    }
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(e) => return (format!("{}: {e}", path.display()), true),
    };
    let count = content.matches(&old).count();
    if count == 0 {
        return (
            format!("old_string not found in {} — read the file and copy the exact text (whitespace matters)", path.display()),
            true,
        );
    }
    let replace_all = args["replace_all"].as_bool().unwrap_or(false);
    if count > 1 && !replace_all {
        return (
            format!("old_string matches {count} times in {} — add surrounding context or set replace_all=true", path.display()),
            true,
        );
    }
    let updated = if replace_all {
        content.replace(&old, &new)
    } else {
        content.replacen(&old, &new, 1)
    };
    match tokio::fs::write(&path, updated.as_bytes()).await {
        Ok(_) => (
            format!("edited {} ({} replacement{})", path.display(), count, if count == 1 { "" } else { "s" }),
            false,
        ),
        Err(e) => (format!("{}: {e}", path.display()), true),
    }
}

fn glob_files(args: &Value, env: &Env) -> (String, bool) {
    let pattern = s(args, "pattern");
    if pattern.is_empty() {
        return ("pattern is empty".to_string(), true);
    }
    let base = args["path"].as_str().map(|p| resolve(&env.cwd, p)).unwrap_or_else(|| env.cwd.clone());
    let glob = match GlobBuilder::new(&pattern).literal_separator(false).build() {
        Ok(g) => g,
        Err(e) => return (format!("bad glob: {e}"), true),
    };
    let matcher = glob.compile_matcher();
    let mut hits: Vec<(std::time::SystemTime, String)> = Vec::new();
    for entry in ignore::WalkBuilder::new(&base).hidden(false).git_ignore(true).build().flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(&base).unwrap_or(path).to_string_lossy().replace('\\', "/");
        if matcher.is_match(&rel) {
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            hits.push((mtime, rel));
        }
    }
    if hits.is_empty() {
        return (format!("no files match '{pattern}' under {}", base.display()), false);
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0));
    let total = hits.len();
    let mut out: Vec<String> = hits.into_iter().take(200).map(|(_, p)| p).collect();
    out.sort();
    let mut text = out.join("\n");
    if total > 200 {
        text.push_str(&format!("\n… {} more", total - 200));
    }
    (text, false)
}

fn grep_files(args: &Value, env: &Env) -> (String, bool) {
    let pattern = s(args, "pattern");
    if pattern.is_empty() {
        return ("pattern is empty".to_string(), true);
    }
    let re = match RegexBuilder::new(&pattern)
        .case_insensitive(args["ignore_case"].as_bool().unwrap_or(false))
        .build()
    {
        Ok(r) => r,
        Err(e) => return (format!("bad regex: {e}"), true),
    };
    let base = args["path"].as_str().map(|p| resolve(&env.cwd, p)).unwrap_or_else(|| env.cwd.clone());
    let file_glob = match args["glob"].as_str() {
        Some(g) if !g.is_empty() => match GlobBuilder::new(g).literal_separator(false).build() {
            Ok(g) => Some(g.compile_matcher()),
            Err(e) => return (format!("bad glob: {e}"), true),
        },
        _ => None,
    };
    let max = args["max_results"].as_u64().unwrap_or(200).max(1) as usize;
    let mut out: Vec<String> = Vec::new();

    let walk = if base.is_file() {
        ignore::WalkBuilder::new(&base).build()
    } else {
        ignore::WalkBuilder::new(&base).hidden(false).git_ignore(true).build()
    };
    for entry in walk.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let rel = if base.is_file() {
            path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
        } else {
            path.strip_prefix(&base).unwrap_or(path).to_string_lossy().replace('\\', "/")
        };
        if let Some(m) = &file_glob {
            if !m.is_match(&rel) {
                continue;
            }
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > 5_000_000 {
            continue;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bytes.iter().take(8000).any(|b| *b == 0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        for (i, line) in text.lines().enumerate() {
            if re.is_match(line) {
                out.push(format!("{}:{}: {}", rel, i + 1, line.trim_end()));
                if out.len() >= max {
                    break;
                }
            }
        }
        if out.len() >= max {
            break;
        }
    }
    if out.is_empty() {
        return (format!("no matches for '{pattern}'"), false);
    }
    let mut text = out.join("\n");
    if text.lines().count() >= max {
        text.push_str(&format!("\n… capped at {max} matches"));
    }
    (text, false)
}

fn list_dir(args: &Value, env: &Env) -> (String, bool) {
    let base = args["path"].as_str().map(|p| resolve(&env.cwd, p)).unwrap_or_else(|| env.cwd.clone());
    let rd = match std::fs::read_dir(&base) {
        Ok(r) => r,
        Err(e) => return (format!("{}: {e}", base.display()), true),
    };
    let mut entries: Vec<(bool, String, u64)> = Vec::new();
    for e in rd.flatten() {
        let meta = e.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        entries.push((is_dir, e.file_name().to_string_lossy().to_string(), size));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut out = String::new();
    for (is_dir, name, size) in entries {
        if is_dir {
            out.push_str(&format!("dir    {}/\n", name));
        } else {
            out.push_str(&format!("{:>6} {}\n", human_size(size), name));
        }
    }
    if out.is_empty() {
        out.push_str("(empty directory)");
    }
    (out, false)
}

fn strip_html(html: &str) -> String {
    let script = regex::Regex::new(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>").unwrap();
    let tags = regex::Regex::new(r"(?s)<[^>]+>").unwrap();
    let blanks = regex::Regex::new(r"\n{3,}").unwrap();
    let text = script.replace_all(html, " ");
    let text = tags.replace_all(&text, " ");
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    blanks.replace_all(&text, "\n\n").trim().to_string()
}

async fn webfetch(args: &Value) -> (String, bool) {
    let url = s(args, "url");
    if url.is_empty() {
        return ("url is empty".to_string(), true);
    }
    let max = args["max_chars"].as_u64().unwrap_or(20_000).clamp(500, 200_000) as usize;
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("abyss/0.1 (terminal agent)")
        .build()
    {
        Ok(c) => c,
        Err(e) => return (format!("http client: {e}"), true),
    };
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return (format!("fetch {url}: {e}"), true),
    };
    let status = resp.status();
    let is_html = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("html"))
        .unwrap_or(false);
    let body = match resp.text().await {
        Ok(t) => t,
        Err(e) => return (format!("read body: {e}"), true),
    };
    let text = if is_html { strip_html(&body) } else { body };
    (
        format!("HTTP {}\n{}", status.as_u16(), truncate_mid(&text, max)),
        !status.is_success(),
    )
}

fn todo_write(args: &Value, env: &Env) -> (String, bool) {
    let items = args["todos"].as_array().cloned().unwrap_or_default();
    let mut list = env.todos.lock().unwrap();
    list.clear();
    for it in items {
        list.push(TodoItem {
            content: it["content"].as_str().unwrap_or("").to_string(),
            status: it["status"].as_str().unwrap_or("pending").to_string(),
        });
    }
    let mut out = String::new();
    for t in list.iter() {
        let mark = match t.status.as_str() {
            "completed" => "[x]",
            "in_progress" => "[~]",
            _ => "[ ]",
        };
        out.push_str(&format!("{} {}\n", mark, t.content));
    }
    if out.is_empty() {
        out.push_str("(todo list cleared)");
    }
    (out, false)
}

async fn task(args: &Value, env: &Env) -> (String, bool) {
    if env.depth >= 3 {
        return ("max subagent depth (3) reached".to_string(), true);
    }
    let prompt = s(args, "prompt");
    if prompt.trim().is_empty() {
        return ("prompt is empty".to_string(), true);
    }
    let turns = args["max_turns"].as_u64().unwrap_or(env.max_turns as u64).max(1) as u32;
    let fut = crate::agent::run_once(
        env.client.clone(),
        env.system.clone(),
        prompt,
        turns,
        env.permission,
        env.cwd.clone(),
        env.depth + 1,
    );
    match Box::pin(fut).await {
        Ok(t) => {
            if t.trim().is_empty() {
                ("(subagent returned nothing)".to_string(), false)
            } else {
                (t, false)
            }
        }
        Err(e) => (format!("subagent failed: {e:#}"), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_mid_keeps_both_ends() {
        let s = "a".repeat(1000);
        let t = truncate_mid(&s, 100);
        assert!(t.starts_with("aaaa"));
        assert!(t.ends_with("aaaa"));
        assert!(t.contains("chars cut"));
        assert_eq!(truncate_mid("short", 100), "short");
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human_size(10), "10B");
        assert_eq!(human_size(2048), "2.0K");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0M");
    }

    #[test]
    fn resolve_joins_relative() {
        let cwd = Path::new("/tmp/x");
        assert_eq!(resolve(cwd, "a/b"), PathBuf::from("/tmp/x/a/b"));
        assert_eq!(resolve(cwd, "/abs"), PathBuf::from("/abs"));
    }

    #[test]
    fn specs_are_unique_and_well_formed() {
        let specs = specs();
        assert!(specs.len() >= 10);
        let mut names: Vec<&str> = specs.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), specs.len());
        for s in specs {
            assert_eq!(s.parameters["type"], "object");
        }
    }
}
