//! Dispatch CLI → D-Bus or local daemon control.

use std::collections::HashMap;
use std::io;

use clap::CommandFactory;
use clap_complete::{generate, Shell as CompleteShell};

use anyhow::Context;
use crate::cli::{AccountsCmd, Cli, Command, DaemonCmd, FoldersCmd, Shell, SyncCmd};
use crate::config;
use crate::daemon_ctl;
use gds_core::db::{create_pool_from_path, run_migrations, upsert_oauth_app_credentials};
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
        || s.contains("ServiceUnknown")
        || s.contains("Connection refused")
}

fn eprint_daemon_missing(out: &GlobalOut<'_>) {
    out.eprintln(
        "The sync daemon (gds-daemon) is not running — nothing can talk to Google until it is.",
    );
    out.eprintln("");
    out.eprintln("Start it, then run this command again:");
    out.eprintln("  gdrivesync daemon start");
    out.eprintln("  # or from the built tree:");
    out.eprintln("  ./gds-daemon");
    out.eprintln("");
    out.eprintln("(OAuth opens in your browser only after the daemon is on D-Bus.)");
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
        Command::Daemon(DaemonCmd::Restart) => {
            daemon_ctl::daemon_stop()?;
            daemon_ctl::daemon_start(cli.verbose).await?;
            if !cli.quiet {
                out.println("Daemon restarted.");
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
        Command::Configure => {
            if !cli.quiet {
                out.println("Google OAuth credentials (from Cloud Console → Credentials → OAuth client ID, Desktop).");
                out.println("");
            }
            let client_id: String = dialoguer::Input::new()
                .with_prompt("Client ID")
                .allow_empty(false)
                .interact_text()
                .map_err(|e| anyhow::anyhow!("input: {}", e))?;
            let client_secret: String = dialoguer::Password::new()
                .with_prompt("Client secret (input hidden for security — stored in the daemon database)")
                .allow_empty_password(false)
                .interact()
                .map_err(|e| anyhow::anyhow!("password input: {}", e))?;

            let data_dir = config::data_dir();
            std::fs::create_dir_all(&data_dir)
                .with_context(|| format!("create data dir {}", data_dir.display()))?;
            let db_path = data_dir.join("state.db");
            let pool = create_pool_from_path(&db_path)
                .await
                .context("open daemon database")?;
            run_migrations(&pool).await.context("run migrations")?;
            upsert_oauth_app_credentials(&pool, client_id.trim(), &client_secret)
                .await
                .context("save credentials to database")?;

            if !cli.quiet {
                out.println("");
                out.println("OAuth credentials saved to the daemon database.");
            }
            if daemon_ctl::daemon_listening().await {
                daemon_ctl::daemon_stop()?;
                daemon_ctl::daemon_start(cli.verbose).await?;
                if !cli.quiet {
                    out.println("Daemon restarted.");
                }
            } else if !cli.quiet {
                out.println("Daemon was not running; start it with 'gdrivesync daemon start' when needed.");
            }
            return Ok(0);
        }
        _ => {}
    }

    // `accounts add` needs the daemon for OAuth; try to start it if missing (dev: sibling gds-daemon).
    let want_add_account = matches!(&cli.command, Command::Accounts(AccountsCmd::Add));
    if want_add_account && !daemon_ctl::daemon_listening().await {
        if !cli.quiet {
            out.println("gds-daemon is not running — starting it…");
        }
        if let Err(e) = daemon_ctl::daemon_start(cli.verbose).await {
            out.eprintln(&format!("Could not start daemon: {e}"));
            eprint_daemon_missing(&out);
            return Ok(EXIT_ERROR);
        }
        if !cli.quiet {
            out.println("Daemon is up. Continuing with sign-in…");
        }
    }

    let target = BusTarget::from_env();
    let client = match DaemonClient::connect(target.clone()).await {
        Ok(c) => c,
        Err(e) => {
            out.eprintln(&format!("D-Bus: {e}"));
            eprint_daemon_missing(&out);
            return Ok(EXIT_DAEMON_GONE);
        }
    };
    let p = match client.proxy().await {
        Ok(p) => p,
        Err(e) => {
            if is_unreachable(&e) {
                out.eprintln(&format!("daemon not reachable: {e}"));
                eprint_daemon_missing(&out);
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
                        eprint_daemon_missing(&out);
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
                out.println("Opening browser for Google sign-in (daemon handles OAuth)…");
            }
            if let Err(e) = p.add_account().await {
                let msg = e.to_string();
                if is_unreachable(&e) {
                    out.eprintln(&format!("daemon not reachable: {e}"));
                    eprint_daemon_missing(&out);
                    return Ok(EXIT_DAEMON_GONE);
                }
                if msg.contains("OAuth credentials not configured")
                    || msg.contains("client_secret is missing")
                    || (msg.contains("client_secret") && msg.contains("missing"))
                {
                    out.eprintln(&msg);
                    out.eprintln("");
                    out.eprintln("Run 'gdrivesync configure' to set Client ID and Client Secret, then restart the daemon.");
                    return Ok(EXIT_ERROR);
                }
                return Err(map_zbus_err(e));
            }
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
        Command::Daemon(_) | Command::Completions { .. } | Command::Configure => unreachable!(),
    }

    Ok(0)
}
