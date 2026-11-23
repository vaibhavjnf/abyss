use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

static COLOR: AtomicBool = AtomicBool::new(true);

pub fn init(no_color: bool) {
    let tty = std::io::stdout().is_terminal();
    let env_off = std::env::var_os("NO_COLOR").is_some();
    COLOR.store(!no_color && tty && !env_off, Ordering::Relaxed);
    #[cfg(windows)]
    enable_vt();
}

pub fn color_on() -> bool {
    COLOR.load(Ordering::Relaxed)
}

fn paint(code: &str, s: &str) -> String {
    if color_on() {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

pub fn dim(s: &str) -> String { paint("2", s) }
pub fn bold(s: &str) -> String { paint("1", s) }
pub fn cyan(s: &str) -> String { paint("36", s) }
pub fn green(s: &str) -> String { paint("32", s) }
pub fn red(s: &str) -> String { paint("31", s) }
pub fn yellow(s: &str) -> String { paint("33", s) }
pub fn magenta(s: &str) -> String { paint("35", s) }

pub fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n).collect();
    out.push('…');
    out
}

pub fn one_line(s: &str, n: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&flat, n)
}

pub fn banner(version: &str, model: &str, provider: &str, cwd: &str, perm: &str) {
    println!();
    println!("{}", cyan("  ▄▀█ █▄▄ █▄█ █▀ █▀"));
    println!("{}", cyan("  █▀█ █▄█  █  ▄█ ▄█"));
    println!("  {} {}", bold("ABYSS"), dim(&format!("v{} — abliterated agent harness", version)));
    println!("  {} {}  {} {}", dim("model"), bold(model), dim("provider"), provider);
    println!("  {} {}  {} {}", dim("cwd"), cwd, dim("mode"), perm);
    println!("  {}", dim("no leash. no morals. just work.  (/help for commands)"));
    println!();
}

pub fn prompt() -> String {
    format!("{} ", cyan("❯"))
}

pub fn info(msg: &str) {
    println!("{} {}", cyan("•"), msg);
}

pub fn ok(msg: &str) {
    println!("{} {}", green("✔"), msg);
}

pub fn warn(msg: &str) {
    println!("{} {}", yellow("!"), msg);
}

pub fn error(msg: &str) {
    eprintln!("{} {}", red("✘"), msg);
}

pub fn stream_delta(s: &str) {
    print!("{}", s);
    let _ = std::io::stdout().flush();
}

pub fn newline() {
    println!();
}

pub fn tool_call(name: &str, args: &str) {
    println!("\n  {} {}{}", cyan("⏺"), bold(name), dim(&format!("({})", one_line(args, 110))));
}

pub fn tool_result(out: &str, is_err: bool, quiet: bool) {
    if quiet {
        return;
    }
    let lines: Vec<&str> = out.lines().collect();
    let shown = if lines.len() > 8 {
        format!("{}\n{}", lines[..6].join("\n"), dim(&format!("… {} more lines", lines.len() - 6)))
    } else {
        out.to_string()
    };
    let shown = truncate_chars(&shown, 900);
    for (i, line) in shown.lines().enumerate() {
        let body = if is_err { red(line) } else { dim(line) };
        if i == 0 {
            println!("  {} {}", dim("⎿"), body);
        } else {
            println!("    {}", body);
        }
    }
    if shown.is_empty() {
        println!("  {} {}", dim("⎿"), dim("(no output)"));
    }
}

pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    pub fn start(label: &str) -> Spinner {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let label = label.to_string();
        let animate = std::io::stdout().is_terminal();
        let handle = std::thread::spawn(move || {
            if !animate {
                return;
            }
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0usize;
            while !flag.load(Ordering::Relaxed) {
                print!("\r\x1b[2K{} {}", paint("36", frames[i % frames.len()]), paint("2", &label));
                let _ = std::io::stdout().flush();
                i += 1;
                std::thread::sleep(Duration::from_millis(80));
            }
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        });
        Spinner { stop, handle: Some(handle) }
    }

    pub fn stop(self) {}
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(windows)]
fn enable_vt() {
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };
    unsafe {
        let h = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if GetConsoleMode(h, &mut mode) != 0 {
            SetConsoleMode(h, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}
