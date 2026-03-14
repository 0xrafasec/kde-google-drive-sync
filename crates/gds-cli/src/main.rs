//! Google Drive sync CLI — `gdrivesync`.

use clap::Parser;
use gds_cli::{run, Cli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(if cli.verbose {
                "info".parse().unwrap()
            } else {
                "warn".parse().unwrap()
            }),
        )
        .init();

    match run::run(cli).await {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {:#}", e);
            std::process::exit(run::EXIT_ERROR);
        }
    }
}
