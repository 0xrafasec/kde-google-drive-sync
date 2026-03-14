//! Clap definition — shared by binary, man generation, and completions.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "gdrivesync",
    about = "Google Drive sync — CLI (talks to gds-daemon via D-Bus)",
    version,
    arg_required_else_help = false
)]
pub struct Cli {
    /// Machine-readable JSON on stdout (stable field names).
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-error stdout.
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Extra diagnostics (implies info-level logging if RUST_LOG unset).
    #[arg(long, short, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Per-account status, quota, last sync (requires running daemon).
    Status,
    #[command(subcommand)]
    Accounts(AccountsCmd),
    #[command(subcommand)]
    Sync(SyncCmd),
    #[command(subcommand)]
    Folders(FoldersCmd),
    /// Recent sync errors from daemon DB.
    Errors,
    /// Drive storage quota per account.
    Quota,
    #[command(subcommand)]
    Daemon(DaemonCmd),
    /// Print shell completions (bash, zsh, fish).
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Interactive setup: prompt for Client ID and Client Secret, store in the daemon database.
    Configure,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Subcommand, Debug)]
pub enum AccountsCmd {
    /// List configured accounts.
    List,
    /// Run OAuth via daemon (browser opens); blocks until done.
    Add,
    /// Remove account and revoke token. Asks for confirmation unless --yes.
    Remove {
        id: String,
        #[arg(long, short)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SyncCmd {
    /// Pause all sync folders.
    Pause,
    /// Resume sync.
    Resume,
    /// Queue immediate sync (optional path = restrict to folder under path).
    Now { path: Option<String> },
}

#[derive(Subcommand, Debug)]
pub enum FoldersCmd {
    /// List sync folder mappings.
    List,
    /// Add mapping (uses first account; see daemon docs).
    Add {
        local_path: String,
        drive_folder_id: String,
    },
    Remove {
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum DaemonCmd {
    /// Start daemon (systemd --user if available, else spawn gds-daemon).
    Start,
    /// Stop daemon (SIGTERM to PID file or systemctl).
    Stop,
    /// Restart daemon (stop then start).
    Restart,
    /// Show whether daemon is on D-Bus and PID file.
    Status,
}
