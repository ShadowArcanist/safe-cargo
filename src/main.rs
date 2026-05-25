mod cache;
mod cargo;
mod cli;
mod commands;
mod config;
mod error;
mod github;
mod keychain;
mod policy;
mod reports;
mod resolver;
mod signing;
mod validate;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Setup => commands::setup::run(),
        Commands::Add { ref crate_spec } => commands::add::run(crate_spec),
        Commands::Check => commands::check::run(),
        Commands::Status => commands::status::run(),
        Commands::Allow {
            ref crate_spec,
            ref reason,
        } => commands::allow::run(crate_spec, reason),
        Commands::Audit => commands::audit::run(),
        Commands::Update => {
            // Backup Cargo.lock before mutation
            let lock_path = std::env::current_dir().ok().map(|d| d.join("Cargo.lock"));
            let lock_backup = lock_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok());

            if let Err(e) = cargo::run(&["update"]) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }

            match commands::check::run() {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Restore Cargo.lock on check failure using atomic write
                    if let (Some(path), Some(backup)) = (&lock_path, &lock_backup) {
                        let tmp_path =
                            path.with_extension(format!("lock.tmp.{}", std::process::id()));
                        if std::fs::write(&tmp_path, backup).is_ok()
                            && std::fs::rename(&tmp_path, path).is_ok()
                        {
                            eprintln!("Cargo.lock restored to pre-update state.");
                        }
                    }
                    Err(e)
                }
            }
        }
        Commands::Passthrough(args) => (|| -> error::Result<()> {
            if let Some(subcmd) = args.first().map(|s| s.as_str()) {
                match subcmd {
                    "install" | "add" => {
                        eprintln!("error: use `safe-cargo add` instead of `safe-cargo {subcmd}` for supply-chain protection");
                        std::process::exit(1);
                    }
                    "build" | "run" | "test" | "bench" | "check" => {
                        eprintln!("Running `safe-cargo check` before `cargo {subcmd}`...");
                        // Run security check first, then forward to cargo
                        commands::check::run()?;
                        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                        cargo::run(&str_args)?;
                    }
                    _ => {
                        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                        cargo::run(&str_args)?;
                    }
                }
            }
            Ok(())
        })(),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
