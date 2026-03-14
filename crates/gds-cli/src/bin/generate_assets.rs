//! Write man page + shell completions into `crates/gds-cli/assets/`.
//! Run: `cargo run -p gds-cli --bin gds-cli-generate`

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::{generate as gen, Shell};
use gds_cli::Cli;

fn main() -> anyhow::Result<()> {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    fs::create_dir_all(&out_dir)?;

    let cmd = Cli::command();
    let man = clap_mangen::Man::new(cmd.clone());
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    fs::write(out_dir.join("gdrivesync.1"), buf)?;

    for (shell, name) in [
        (Shell::Bash, "gdrivesync.bash"),
        (Shell::Zsh, "_gdrivesync"),
        (Shell::Fish, "gdrivesync.fish"),
    ] {
        let mut v = Vec::new();
        gen(shell, &mut Cli::command(), "gdrivesync", &mut v);
        fs::write(out_dir.join(name), v)?;
    }

    let mut w = std::io::stdout();
    writeln!(w, "Wrote man + completions to {}", out_dir.display())?;
    Ok(())
}
