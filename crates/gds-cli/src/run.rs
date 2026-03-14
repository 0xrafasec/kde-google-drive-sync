//! Dispatch CLI → D-Bus or local daemon control.

use std::collections::HashMap;
use std::io;

use clap::CommandFactory;
use clap_complete::{generate, Shell as CompleteShell};

use crate::cli::{AccountsCmd, Cli, Command, DaemonCmd, FoldersCmd, Shell, SyncCmd};
use crate::daemon_ctl;
use crate::dbus_client::{BusTarget, DaemonClient};
use crate::output::{self, GlobalOut, StatusJson};

/// Exit code: daemon not reachable on D-Bus.
pub const EXIT_DAEMON_GONE: i32 = 2;
pub const EXIT_ERROR: i32 = 1;

fn map_zbus_err(e: zbus::Error) -> anyhow::Error {
    anyhow::anyhow!("{}", e)
}

fn is_unreachable(e: &zbus::Error) -> bool {
    let s = e.to_string();
    s.contains("Name has no owner")
        || s.contains("name not found")
        || s.contains("Could not get PM")
        || s.contains("The name is not activatable")
        || s.contains("Connection refused")
}

pub async fn run(cli: Cli) -> anyhow::Result<i32> {
    if cli.verbose && std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }

    let out = GlobalOut { cli: &cli };

    match &cli.command {
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin = "gdrivesync";
            match shell {
                Shell::Bash => generate(CompleteShell::Bash, &mut cmd, bin, &mut io::stdout()),
                Shell::Zsh => generate(CompleteShell::Zsh, &mut cmd, bin, &mut io::stdout()),
                Shell::Fish => generate(CompleteShell::Fish, &mut cmd, bin, &mut io::stdout()),
            }
            return Ok(0);
        }
        Command::Daemon(DaemonCmd::Start) => {
            daemon_ctl::daemon_start(cli.verbose).await?;
            if !cli.quiet {
                out.println("Daemon start requested.");
            }
            return Ok(0);
        }
        Command::Daemon(DaemonCmd::Stop) => {
            daemon_ctl::daemon_stop()?;
            if !cli.quiet {
                out.println("Daemon stop sent.");
            }
            return Ok(0);
        }
        Command::Daemon(DaemonCmd::Status) => {
            let target = BusTarget::from_env();
            let j = daemon_ctl::daemon_status_json(&target).await;
            if cli.json {
                out.json(&j)?;
            } else {
                out.println(&format!(
                    "on_bus={}  service={}  pid={:?}  pid_file={:?}",
                    j.on_bus, j.service, j.pid, j.pid_file
                ));
            }
            return Ok(0);
        }
        _ => {}
    }

    let target = BusTarget::from_env();
    let client = match DaemonClient::connect(target.clone()).await {
        Ok(c) => c,
        Err(e) => {
            out.eprintln(&format!("D-Bus: {e}"));
            return Ok(EXIT_DAEMON_GONE);
        }
    };
    let p = match client.proxy().await {
        Ok(p) => p,
        Err(e) => {
            if is_unreachable(&e) {
                out.eprintln(&format!("daemon not reachable: {e}"));
                return Ok(EXIT_DAEMON_GONE);
            }
            return Err(map_zbus_err(e));
        }
    };

    macro_rules! call {
        ($e:expr) => {
            match $e {
                Ok(x) => x,
                Err(e) => {
                    if is_unreachable(&e) {
                        out.eprintln(&format!("daemon not reachable: {e}"));
                        return Ok(EXIT_DAEMON_GONE);
                    }
                    return Err(map_zbus_err(e));
                }
            }
        };
    }

    match &cli.command {
        Command::Status => {
            let (global_status, syncing_count) = call!(p.get_status().await);
            let accounts = call!(p.get_accounts().await);
            let folders = call!(p.get_sync_folders().await);
            let mut quota_by_account = HashMap::new();
            for a in &accounts {
                if let Ok(q) = p.get_about_info(a.id.as_str()).await {
                    quota_by_account.insert(a.id.clone(), q);
                }
            }
            if cli.json {
                let j: StatusJson = output::status_json(
                    global_status,
                    syncing_count,
                    &accounts,
                    &folders,
                    &quota_by_account,
                );
                out.json(&j)?;
            } else {
                output::status_human(
                    &out,
                    &global_status,
                    syncing_count,
                    &accounts,
                    &folders,
                    &quota_by_account,
                );
            }
        }
        Command::Accounts(AccountsCmd::List) => {
            let accounts = call!(p.get_accounts().await);
            if cli.json {
                out.json(&accounts)?;
            } else {
                output::accounts_list_human(&out, &accounts);
            }
        }
        Command::Accounts(AccountsCmd::Add) => {
            if !cli.quiet {
                out.println("Opening browser for OAuth (via daemon)…");
            }
            call!(p.add_account().await);
            if !cli.quiet {
                out.println("Account added.");
            }
        }
        Command::Accounts(AccountsCmd::Remove { id, yes }) => {
            if !*yes {
                let proceed = dialoguer::Confirm::new()
                    .with_prompt(format!("Remove account {id} and revoke tokens?"))
                    .default(false)
                    .interact()?;
                if !proceed {
                    out.eprintln("Aborted.");
                    return Ok(0);
                }
            }
            call!(p.remove_account(id.as_str()).await);
            if !cli.quiet {
                out.println("Account removed.");
            }
        }
        Command::Sync(SyncCmd::Pause) => {
            call!(p.pause_sync().await);
            if !cli.quiet {
                out.println("Sync paused.");
            }
        }
        Command::Sync(SyncCmd::Resume) => {
            call!(p.resume_sync().await);
            if !cli.quiet {
                out.println("Sync resumed.");
            }
        }
        Command::Sync(SyncCmd::Now { path }) => {
            let path_s = path.as_deref().unwrap_or("");
            call!(p.force_sync(path_s).await);
            if !cli.quiet {
                out.println("Sync queued.");
            }
        }
        Command::Folders(FoldersCmd::List) => {
            let folders = call!(p.get_sync_folders().await);
            if cli.json {
                out.json(&folders)?;
            } else {
                output::folders_list_human(&out, &folders);
            }
        }
        Command::Folders(FoldersCmd::Add {
            local_path,
            drive_folder_id,
        }) => {
            call!(
                p.add_sync_folder(local_path.as_str(), drive_folder_id.as_str())
                    .await
            );
            if !cli.quiet {
                out.println("Sync folder added (first account).");
            }
        }
        Command::Folders(FoldersCmd::Remove { id }) => {
            call!(p.remove_sync_folder(id.as_str()).await);
            if !cli.quiet {
                out.println("Sync folder removed.");
            }
        }
        Command::Errors => {
            let errors = call!(p.get_sync_errors().await);
            if cli.json {
                out.json(&errors)?;
            } else {
                output::errors_human(&out, &errors);
            }
        }
        Command::Quota => {
            let accounts = call!(p.get_accounts().await);
            let mut quota: HashMap<String, crate::dbus_types::QuotaInfo> = HashMap::new();
            for a in &accounts {
                if let Ok(q) = p.get_about_info(a.id.as_str()).await {
                    quota.insert(a.id.clone(), q);
                }
            }
            if cli.json {
                let v: Vec<_> = accounts
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "id": a.id,
                            "email": a.email,
                            "quota": quota.get(&a.id),
                        })
                    })
                    .collect();
                out.json(&v)?;
            } else {
                output::quota_human(&out, &accounts, &quota);
            }
        }
        Command::Daemon(_) | Command::Completions { .. } => unreachable!(),
    }

    Ok(0)
}
