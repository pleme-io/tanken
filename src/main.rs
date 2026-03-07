mod config;
mod platform;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use crate::config::TankenConfig;

#[derive(Parser)]
#[command(name = "tanken", about = "Tanken (探検) — GPU file manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the background file-watching daemon.
    Daemon,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // Load config via shikumi
    let config = match shikumi::ConfigDiscovery::new("tanken")
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
    };

    match cli.command {
        Some(Command::Daemon) => {
            tracing::info!("starting tanken daemon");
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(async {
                tracing::info!(
                    "daemon watching {} directories, interval {}s",
                    config.daemon.watch_dirs.len(),
                    config.daemon.index_interval_secs
                );
                // Daemon event loop will be implemented here
                tokio::signal::ctrl_c()
                    .await
                    .expect("failed to listen for ctrl-c");
                tracing::info!("daemon shutting down");
            });
        }
        None => {
            tracing::info!("launching tanken GUI");
            tracing::info!(
                "default path: {}",
                config.navigation.default_path.display()
            );
            // GUI event loop will be implemented here
        }
    }
}
