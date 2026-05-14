// Install mimalloc as the global allocator via moldura — fleet-
// canonical. Free perf on tanken's directory-walking + preview
// rendering paths.
moldura::mimalloc_init!();

mod app;
mod bookmarks;
mod config;
mod fs;
mod input;
mod mcp;
mod pane;
mod platform;
mod preview;
mod render;
mod search;
mod tabs;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::event::{self, Event};
use egaku_term::Terminal;
use tracing_subscriber::EnvFilter;

use crate::app::App;
use crate::config::TankenConfig;

#[derive(Parser)]
#[command(name = "tanken", about = "Tanken (探検) — GPU file manager")]
struct Cli {
    /// Directory to open.
    #[arg(default_value = None)]
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the background file-watching daemon.
    Daemon,
    /// Run as MCP server (stdio transport) for Claude Code integration.
    Mcp,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(io::stderr)
        .init();

    let cli = Cli::parse();

    // Handle MCP subcommand before loading TUI config
    if let Some(Command::Mcp) = cli.command {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(mcp::run())?;
        return Ok(());
    }

    // Load config via shikumi
    let config = load_config();

    match cli.command {
        Some(Command::Mcp) => unreachable!("handled above"),
        Some(Command::Daemon) => {
            tracing::info!("starting tanken daemon");
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(async {
                tracing::info!(
                    "daemon watching {} directories, interval {}s",
                    config.daemon.watch_dirs.len(),
                    config.daemon.index_interval_secs
                );
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to listen for ctrl-c");
                tracing::info!("daemon shutting down");
            });
            Ok(())
        }
        None => {
            let initial_path = cli.path.unwrap_or_else(|| {
                config.navigation.default_path.clone()
            });

            let initial_path = if initial_path.is_relative() {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("/"))
                    .join(initial_path)
            } else {
                initial_path
            };

            run_tui(config, initial_path)
        }
    }
}

fn load_config() -> TankenConfig {
    match shikumi::ConfigDiscovery::new("tanken")
        .env_override("TANKEN_CONFIG")
        .discover()
    {
        Ok(path) => {
            tracing::info!("loading config from {}", path.display());
            let store =
                shikumi::ConfigStore::<TankenConfig>::load(&path, "TANKEN_").unwrap_or_else(|e| {
                    tracing::warn!("failed to load config: {e}, using defaults");
                    let tmp = std::env::temp_dir().join("tanken-default.yaml");
                    std::fs::write(&tmp, "{}").ok();
                    shikumi::ConfigStore::load(&tmp, "TANKEN_").unwrap()
                });
            TankenConfig::clone(&store.get())
        }
        Err(_) => {
            tracing::info!("no config file found, using defaults");
            TankenConfig::default()
        }
    }
}

fn run_tui(config: TankenConfig, initial_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // egaku-term owns terminal lifecycle (raw mode + alt screen + hide
    // cursor + Drop-safe restore on panic).
    let mut term = Terminal::enter()?;
    let mut app = App::new(config, initial_path);
    run_event_loop(&mut term, &mut app)
}

fn run_event_loop(
    term: &mut Terminal,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        term.clear()?;
        render::draw(term, app)?;
        term.flush()?;

        if app.should_quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                let action = app.input.handle_key(key_event);
                app.process_action(action);
            }
        }
    }
}

// `io` is referenced indirectly via egaku_term::Terminal; the import stays
// to keep error-conversion paths tidy.
const _: fn() = || {
    let _ = std::mem::size_of::<io::Error>();
};
