mod agent;
mod cli;
mod config;
mod llm;
mod prompt;
mod session;
mod skills;
mod tools;
mod ui;

#[tokio::main]
async fn main() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let code = match cli::run().await {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("{e:#}"));
            1
        }
    };
    std::process::exit(code);
}
