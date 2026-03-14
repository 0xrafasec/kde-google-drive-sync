//! Google Drive sync CLI — `gdrivesync`.

use clap::{CommandFactory, Parser};
use gds_cli::{run, Cli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        let mut cmd = Cli::command();
        cmd.write_long_help(&mut std::io::stdout())?;
        return Ok(());
    }
    let cli = Cli::parse_from(&args);

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
