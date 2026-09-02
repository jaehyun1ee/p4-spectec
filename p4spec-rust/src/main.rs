use std::{env, ffi::OsStr, path::PathBuf, process::ExitCode};

use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate},
};

const USAGE: &str = "Usage: p4spec-rust <elab|algo> <path>...";

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let Some(command) = args.next() else {
        return usage_error();
    };

    if command == OsStr::new("--help") || command == OsStr::new("-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if command != OsStr::new("elab") && command != OsStr::new("algo") {
        eprintln!("unknown command: {}", command.to_string_lossy());
        return usage_error();
    }

    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    if matches!(paths.as_slice(), [path] if path == OsStr::new("--help") || path == OsStr::new("-h"))
    {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if paths.is_empty() {
        return usage_error();
    }

    let spec_el = match parse_files(paths) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_il = match elaborate::elaborate(spec_el) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };

    if command == OsStr::new("elab") {
        println!("{}", Print::to_string(&spec_il));
        return ExitCode::SUCCESS;
    }

    let spec_al = match algo::convert(spec_il) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    println!("{}", Print::to_string(&spec_al));
    ExitCode::SUCCESS
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn command_error(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}
