mod corpus;
mod progress;
mod run;
mod snapshot;

use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}
type Result<T> = std::result::Result<T, Error>;

/// Native expected tests, independent of the product's unit tests
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compare elaborated specification output and rejected inputs
    Elab,
    /// Compare algorithmic specification output and rejected inputs
    Algo,
    /// Compare the full P4 corpus with stored file results (cache on, det off)
    RunAl,
}

fn execute(command: Command) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    std::env::set_current_dir(&root)?;
    match command {
        Command::Elab => snapshot::run(false),
        Command::Algo => snapshot::run(true),
        Command::RunAl => run::run(),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = std::thread::Builder::new()
        .name("expected-driver".to_owned())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || execute(cli.command));
    match result {
        Ok(thread) => match thread.join() {
            Ok(Ok(())) => ExitCode::SUCCESS,
            Ok(Err(error)) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
            Err(_) => {
                eprintln!("test driver panicked; corpus execution is incomplete");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("cannot start test driver: {error}");
            ExitCode::FAILURE
        }
    }
}
