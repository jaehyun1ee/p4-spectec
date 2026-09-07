use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global},
    lang::traits::print::Print,
    pass::{algo, elaborate},
    runner::{BuiltinInterface, NullExtern, Runner},
};

/// Elaborate and convert P4 specifications
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Elaborate specifications and print the intermediate representation
    Elab(InputArgs),
    /// Convert specifications and print the algorithmic representation
    Algo(InputArgs),
    /// Run a P4 program with the algorithmic interpreter
    Run(RunOptions),
}

#[derive(Args)]
struct InputArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (paths, convert) = match cli.command {
        Command::Run(options) => return run_command(options),
        Command::Elab(InputArgs { paths }) => (paths, false),
        Command::Algo(InputArgs { paths }) => (paths, true),
    };

    let spec_el = match parse_files(paths) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_il = match elaborate::elaborate(spec_el) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };

    if convert {
        let spec_al = match algo::convert(spec_il) {
            Ok(spec) => spec,
            Err(error) => return command_error(error),
        };
        println!("{}", Print::to_string(&spec_al));
    } else {
        println!("{}", Print::to_string(&spec_il));
    }
    ExitCode::SUCCESS
}

fn command_error(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}

#[derive(Args)]
struct RunOptions {
    /// Execute the algorithmic representation
    #[arg(long, required = true)]
    al: bool,
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Entry relation to evaluate
    #[arg(long = "rel", value_name = "RELATION")]
    relation: String,
    /// P4 program to execute
    #[arg(short = 'p', value_name = "PROGRAM")]
    program: PathBuf,
    /// Include directories for the P4 program
    #[arg(short = 'i', value_name = "DIR")]
    includes: Vec<PathBuf>,
    /// Check deterministic execution
    #[arg(long)]
    det: bool,
    /// Check interpreter guards
    #[arg(long)]
    guard: bool,
}

fn run_command(options: RunOptions) -> ExitCode {
    let spec_el = match parse_files(options.paths) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_il = match elaborate::elaborate(spec_el) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let spec_al = match algo::convert(spec_il) {
        Ok(spec) => spec,
        Err(error) => return command_error(error),
    };
    let unparser = P4Unparser::from_al_spec(&spec_al);
    let global = match Global::load(spec_al) {
        Ok(global) => global,
        Err(error) => return command_error(error),
    };
    let program = match parse_file(&options.includes, options.program) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("syntax error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut runner = Runner::<Al, _, _>::new(
        global,
        Config::new(options.det, options.guard),
        BuiltinInterface::new(unparser),
        NullExtern,
    );
    match runner.eval_program(&options.relation, program) {
        Ok(_) => {
            println!("passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("runtime error: {error}");
            ExitCode::FAILURE
        }
    }
}
