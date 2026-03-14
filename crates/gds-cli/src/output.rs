//! Human and JSON output helpers.

use std::collections::HashMap;
use std::io::{self, Write};

use serde::Serialize;

use crate::cli::Cli;
use crate::dbus_types::{AccountInfo, QuotaInfo, SyncErrorInfo, SyncFolderInfo};

pub struct GlobalOut<'a> {
    pub cli: &'a Cli,
}

impl<'a> GlobalOut<'a> {
    pub fn println(&self, s: &str) {
        if !self.cli.quiet {
            let _ = writeln!(io::stdout(), "{s}");
        }
    }

    pub fn eprintln(&self, s: &str) {
        let _ = writeln!(io::stderr(), "{s}");
    }

    pub fn json<T: Serialize>(&self, v: &T) -> anyhow::Result<()> {
        println!("{}", serde_json::to_string_pretty(v)?);
        Ok(())
    }
}

#[derive(Serialize)]
pub struct StatusJson {
    pub global_status: String,
    pub syncing_count: u32,
    pub accounts: Vec<StatusAccountJson>,
}

#[derive(Serialize)]
pub struct StatusAccountJson {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub quota_limit: String,
    pub quota_usage: String,
    pub last_sync_unix: Option<i64>,
}

pub fn status_human(
    out: &GlobalOut<'_>,
    global_status: &str,
    syncing_count: u32,
    accounts: &[AccountInfo],
    folders: &[SyncFolderInfo],
    quota_by_account: &HashMap<String, QuotaInfo>,
) {
    out.println(&format!(
        "Status: {global_status} (active syncs: {syncing_count})"
    ));
    out.println("");
    for a in accounts {
        let mut last: Option<i64> = None;
        for f in folders {
            if f.account_id == a.id && f.last_sync_at > 0 {
                last = Some(match last {
                    None => f.last_sync_at,
                    Some(x) => x.max(f.last_sync_at),
                });
            }
        }
        let q = quota_by_account.get(&a.id);
        let (lim, use_s) = match q {
            Some(q) => (q.limit.as_str(), q.usage.as_str()),
            None => ("-", "-"),
        };
        let last_s = last
            .map(|t| {
                chrono::DateTime::from_timestamp(t, 0)
                    .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| t.to_string())
            })
            .unwrap_or_else(|| "-".to_string());
        out.println(&format!(
            "Account {} ({})",
            a.email.if_empty(&a.id),
            &a.id[..8.min(a.id.len())]
        ));
        out.println(&format!("  display: {}", a.display_name));
        out.println(&format!("  quota: {use_s} / {lim}"));
        out.println(&format!("  last sync: {last_s}"));
        out.println("");
    }
}

pub fn status_json(
    global_status: String,
    syncing_count: u32,
    accounts: &[AccountInfo],
    folders: &[SyncFolderInfo],
    quota_by_account: &HashMap<String, QuotaInfo>,
) -> StatusJson {
    let accounts_json: Vec<StatusAccountJson> = accounts
        .iter()
        .map(|a| {
            let mut last: Option<i64> = None;
            for f in folders {
                if f.account_id == a.id && f.last_sync_at > 0 {
                    last = Some(match last {
                        None => f.last_sync_at,
                        Some(x) => x.max(f.last_sync_at),
                    });
                }
            }
            let q = quota_by_account.get(&a.id);
            StatusAccountJson {
                id: a.id.clone(),
                email: a.email.clone(),
                display_name: a.display_name.clone(),
                quota_limit: q.map(|x| x.limit.clone()).unwrap_or_default(),
                quota_usage: q.map(|x| x.usage.clone()).unwrap_or_default(),
                last_sync_unix: last,
            }
        })
        .collect();
    StatusJson {
        global_status,
        syncing_count,
        accounts: accounts_json,
    }
}

trait StrOrId {
    fn if_empty(&self, fallback: &str) -> String;
}
impl StrOrId for str {
    fn if_empty(&self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self.to_string()
        }
    }
}

pub fn accounts_list_human(out: &GlobalOut<'_>, accounts: &[AccountInfo]) {
    if accounts.is_empty() {
        out.println("No accounts.");
        return;
    }
    for a in accounts {
        out.println(&format!(
            "{}  id={}  name={}",
            a.email.if_empty("(no email)"),
            a.id,
            a.display_name
        ));
    }
}

pub fn folders_list_human(out: &GlobalOut<'_>, folders: &[SyncFolderInfo]) {
    if folders.is_empty() {
        out.println("No sync folders.");
        return;
    }
    for f in folders {
        let last = if f.last_sync_at > 0 {
            chrono::DateTime::from_timestamp(f.last_sync_at, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| f.last_sync_at.to_string())
        } else {
            "-".to_string()
        };
        out.println(&format!(
            "{}  local={}  drive={}  paused={}  last={}",
            f.id, f.local_path, f.drive_folder_id, f.paused, last
        ));
    }
}

pub fn errors_human(out: &GlobalOut<'_>, errors: &[SyncErrorInfo]) {
    if errors.is_empty() {
        out.println("No recent errors.");
        return;
    }
    for e in errors {
        let t = chrono::DateTime::from_timestamp(e.occurred_at, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| e.occurred_at.to_string());
        out.println(&format!(
            "[{}] retry={}  {}",
            t, e.retry_count, e.error_message
        ));
    }
}

pub fn quota_human(
    out: &GlobalOut<'_>,
    accounts: &[AccountInfo],
    quota: &HashMap<String, QuotaInfo>,
) {
    for a in accounts {
        let q = quota.get(&a.id);
        match q {
            Some(q) => out.println(&format!(
                "{}  usage={}  limit={}  in_drive={}",
                a.email.if_empty(&a.id),
                q.usage,
                q.limit,
                q.usage_in_drive
            )),
            None => out.println(&format!("{}  (quota unavailable)", a.email.if_empty(&a.id))),
        }
    }
}
