use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{AlInterp, Config, context::Global},
    lang::{al, il, traits::print::Print},
    pass::{algo, elaborate},
    runner::{BuiltinInterface, Runner},
    sim_plugin::{build, dummy::Dummy, runner::Error as SimError},
    stf,
};

// = Helpers

fn elab(paths: Vec<PathBuf>) -> Result<il::ast::Spec, ExitCode> {
    let spec_el = parse_files(paths).map_err(command_error)?;
    elaborate::elaborate(spec_el).map_err(command_error)
}

fn algo(paths: Vec<PathBuf>) -> Result<al::ast::Spec, ExitCode> {
    let spec_il = elab(paths)?;
    algo::convert(spec_il).map_err(command_error)
}

// = Elab command

#[derive(Args)]
struct ElabArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn elab_command(args: ElabArgs) -> ExitCode {
    let spec_il = match elab(args.paths) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    println!("{}", Print::to_string(&spec_il));
    ExitCode::SUCCESS
}

// = Algo command

#[derive(Args)]
struct AlgoArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
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

// = Run command

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
    /// Disable AL call caching
    #[arg(long)]
    no_cache: bool,
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
    let mut runner = Runner::<AlInterp, _, _>::new(
        global,
        AlInterp::new(Config::new(!args.no_cache, args.det, args.guard)),
        BuiltinInterface::new(unparser),
        Dummy,
    );
    let program = match parse_file(runner.arena_mut(), &args.includes, args.program) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("syntax error: {error}");
            return ExitCode::FAILURE;
        }
    };
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

// = Sim command

#[derive(Args)]
struct SimArgs {
    /// Execute the algorithmic representation
    #[arg(long, required = true)]
    al: bool,
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Target architecture: ebpf, psa, or v1model
    #[arg(long, value_name = "ARCH")]
    arch: String,
    /// P4 program to simulate
    #[arg(short = 'p', value_name = "PROGRAM")]
    program: PathBuf,
    /// STF test to execute
    #[arg(long, value_name = "STF")]
    stf: PathBuf,
    /// Include directories for the P4 program
    #[arg(short = 'i', value_name = "DIR")]
    includes: Vec<PathBuf>,
    /// Disable AL call caching
    #[arg(long)]
    no_cache: bool,
    /// Check deterministic execution
    #[arg(long)]
    det: bool,
    /// Check interpreter guards
    #[arg(long)]
    guard: bool,
}

fn sim_command(args: SimArgs) -> ExitCode {
    let spec_al = match algo(args.paths) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    let config = Config::new(!args.no_cache, args.det, args.guard);
    let mut simulator = match build::build(spec_al, &args.arch, config) {
        Ok(simulator) => simulator,
        Err(error) => return command_error(error),
    };
    let mut run = match simulator.init_pipe(&args.includes, &args.program) {
        Ok(run) => run,
        Err(error) => return command_error(error),
    };
    let stmts = match stf::parse::parse_file(&args.stf) {
        Ok(stmts) => stmts,
        Err(error) => return command_error(SimError::from(error)),
    };
    for stmt in &stmts {
        match simulator.step(&mut run, stmt) {
            Ok(Some(tx)) => println!("[PASS] Transmitted {tx}"),
            Ok(None) => {}
            Err(error) => return command_error(error),
        }
    }
    if let Err(failure) = run.finish() {
        return command_error(SimError::Stf {
            failure: Box::new(failure),
            span: Default::default(),
        });
    }
    println!("passed");
    ExitCode::SUCCESS
}

// = Entry point

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
    /// Simulate a P4 program and STF test on a target architecture
    Sim(SimArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Elab(args) => elab_command(args),
        Command::Algo(args) => algo_command(args),
        Command::Run(args) => run_command(args),
        Command::Sim(args) => sim_command(args),
    }
}
