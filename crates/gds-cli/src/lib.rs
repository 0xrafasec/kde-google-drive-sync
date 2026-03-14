//! `gdrivesync` CLI library (D-Bus client + argparse).

pub mod cli;
pub mod daemon_ctl;
pub mod dbus_client;
pub mod dbus_types;
pub mod output;
pub mod run;

pub use cli::Cli;
