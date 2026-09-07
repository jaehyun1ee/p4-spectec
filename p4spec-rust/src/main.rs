use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{Al, Config, context::Global},
    lang::{al, il, traits::print::Print},
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
    Elab(ElabArgs),
    /// Convert specifications and print the algorithmic representation
    Algo(AlgoArgs),
    /// Run a P4 program with the algorithmic interpreter
    Run(RunArgs),
}

#[derive(Args)]
struct ElabArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Args)]
struct AlgoArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Elab(args) => elab_command(args),
        Command::Algo(args) => algo_command(args),
        Command::Run(args) => run_command(args),
    }
}

fn elab(paths: Vec<PathBuf>) -> Result<il::ast::Spec, ExitCode> {
    let spec_el = parse_files(paths).map_err(command_error)?;
    elaborate::elaborate(spec_el).map_err(command_error)
}

fn algo(paths: Vec<PathBuf>) -> Result<al::ast::Spec, ExitCode> {
    let spec_il = elab(paths)?;
    algo::convert(spec_il).map_err(command_error)
}

fn elab_command(args: ElabArgs) -> ExitCode {
    let spec_il = match elab(args.paths) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    println!("{}", Print::to_string(&spec_il));
    ExitCode::SUCCESS
}

fn algo_command(args: AlgoArgs) -> ExitCode {
    let spec_al = match algo(args.paths) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    println!("{}", Print::to_string(&spec_al));
    ExitCode::SUCCESS
}

fn command_error(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
}

#[derive(Args)]
struct RunArgs {
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

fn run_command(args: RunArgs) -> ExitCode {
    let spec_al = match algo(args.paths) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    let unparser = P4Unparser::from_al_spec(&spec_al);
    let global = match Global::load(spec_al) {
        Ok(global) => global,
        Err(error) => return command_error(error),
    };
    let program = match parse_file(&args.includes, args.program) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("syntax error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut runner = Runner::<Al, _, _>::new(
        global,
        Config::new(args.det, args.guard),
        BuiltinInterface::new(unparser),
        NullExtern,
    );
    match runner.eval_program(&args.relation, program) {
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
