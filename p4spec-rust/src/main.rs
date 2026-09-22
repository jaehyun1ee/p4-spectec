//! Command-line specification transformation and execution
//!
//! Commands render elaboration warnings before running downstream passes.
//! `run` propagates typed failures to `main`,
//! which renders source reports and chooses the process exit code.

use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};

use p4spec_rust::{
    diagnostic::{RenderConfig, Renderer, Report},
    interface::p4::{error::P4Error, parse::parse_file},
    interp::shared::error::Error as InterpError,
    lang::{al, il, pl, sl},
    lang::{data::value::external::Encoding, traits::print::Print},
    pass,
    runner::{self, BuiltinInterface, Interpreter, Runner},
    sim_plugin::{self, dummy::Dummy},
};

// = Helpers

// - Diagnostic output

/// Renders reports without changing their structured payloads.
fn render_report(report: &Report) {
    let mut renderer = Renderer::new(RenderConfig::default());
    if let Err(error) = renderer.render_to_stderr(report) {
        eprintln!("{report}\ndiagnostic rendering failed: {error}");
    }
}

// = Errors

/// A command failure with its user-facing diagnostic category.
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Spec(#[from] p4spec_rust::Error),
    #[error(transparent)]
    Runner(#[from] runner::BuildError),
    #[error(transparent)]
    Simulator(#[from] sim_plugin::BuildError),
    #[error(transparent)]
    Simulation(#[from] sim_plugin::runner::Error),
    #[error("syntax error: {0}")]
    Syntax(#[from] P4Error),
    #[error("runtime error: {0}")]
    Runtime(#[from] InterpError),
}

// = Specification loading

/// Elaborates source paths and emits warnings before propagating a failure.
fn elaborate(paths: &[PathBuf]) -> Result<il::ast::Spec, CliError> {
    let spec_el = p4spec_rust::parse(paths)?;
    let (result, warnings) = pass::elaborate::convert_with_warnings(spec_el);
    for report in warnings {
        render_report(&report);
    }
    result
        .map_err(p4spec_rust::Error::Elab)
        .map_err(CliError::from)
}

/// Converts source paths to AL after rendering elaboration warnings.
fn algorithmic(paths: &[PathBuf]) -> Result<al::ast::Spec, CliError> {
    let spec_il = elaborate(paths)?;
    pass::algo::convert(spec_il)
        .map_err(p4spec_rust::Error::Algo)
        .map_err(CliError::from)
}

/// Converts source paths to SL after rendering elaboration warnings.
fn structured(paths: &[PathBuf], without_rule_groups: bool) -> Result<sl::ast::Spec, CliError> {
    let spec_al = algorithmic(paths)?;
    pass::structure::convert(spec_al, without_rule_groups)
        .map_err(p4spec_rust::Error::Structure)
        .map_err(CliError::from)
}

/// Converts source paths to PL after rendering elaboration warnings.
fn prose(paths: &[PathBuf]) -> Result<pl::ast::Spec, CliError> {
    let spec_sl = structured(paths, false)?;
    pass::prosify::convert(spec_sl)
        .map_err(p4spec_rust::Error::Prose)
        .map_err(CliError::from)
}

// = Elab command

#[derive(Args)]
struct ElabArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn elab_command(args: ElabArgs) -> Result<(), CliError> {
    let spec_il = elaborate(&args.paths)?;
    println!("{}", Print::to_string(&spec_il));
    Ok(())
}

// = Algo command

#[derive(Args)]
struct AlgoArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn algo_command(args: AlgoArgs) -> Result<(), CliError> {
    let spec_al = algorithmic(&args.paths)?;
    println!("{}", Print::to_string(&spec_al));
    Ok(())
}

// = Struct command

#[derive(Args)]
struct StructArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn struct_command(args: StructArgs) -> Result<(), CliError> {
    let spec_sl = structured(&args.paths, true)?;
    println!("{}", Print::to_string(&spec_sl));
    Ok(())
}

// = Prose command

#[derive(Args)]
struct ProseArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn prose_command(args: ProseArgs) -> Result<(), CliError> {
    let spec_pl = prose(&args.paths)?;
    println!("{}", Print::to_string(&spec_pl));
    Ok(())
}

// = Run command

#[derive(Args)]
#[group(required = true, multiple = false)]
struct InterpreterArgs {
    /// Execute the algorithmic representation
    #[arg(long)]
    al: bool,
    /// Execute the structured representation
    #[arg(long)]
    sl: bool,
    /// Execute the prose representation
    #[arg(long)]
    pl: bool,
}

fn interp_spec(paths: &[PathBuf], interpreter: &InterpreterArgs) -> Result<runner::Spec, CliError> {
    // Each pipeline stops at the language selected by the command
    if interpreter.al {
        algorithmic(paths).map(runner::Spec::Al)
    } else if interpreter.sl {
        structured(paths, true).map(runner::Spec::Sl)
    } else {
        prose(paths).map(runner::Spec::Pl)
    }
}

#[derive(Args)]
struct RunArgs {
    #[command(flatten)]
    interpreter: InterpreterArgs,
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
    /// Disable interpreter call caching
    #[arg(long)]
    no_cache: bool,
    /// Check deterministic execution
    #[arg(long)]
    det: bool,
    /// Check interpreter guards
    #[arg(long)]
    guard: bool,
}

/// Builds the selected interpreter and runs the program entry relation.
fn run_command(args: RunArgs) -> Result<(), CliError> {
    // Convert the specification before assembling its runner
    let spec = interp_spec(&args.paths, &args.interpreter)?;
    let config = runner::Config::new(!args.no_cache, args.det, args.guard);
    // Each runner uses the same P4 frontend and dummy extern implementation
    match spec {
        runner::Spec::Al(spec) => {
            let runner = runner::build_al(spec, config, Dummy)?;
            run_program(runner, &args)
        }
        runner::Spec::Sl(spec) => {
            let runner = runner::build_sl(spec, config, Dummy)?;
            run_program(runner, &args)
        }
        runner::Spec::Pl(spec) => {
            let runner = runner::build_pl(spec, config, Dummy)?;
            run_program(runner, &args)
        }
    }
}

fn run_program<Interp>(
    mut runner: Runner<Interp, BuiltinInterface, Dummy>,
    args: &RunArgs,
) -> Result<(), CliError>
where
    Interp: Interpreter<BuiltinInterface, Dummy, Error = InterpError>,
{
    let program = parse_file(runner.arena_mut(), &args.includes, &args.program)?;
    runner.eval_program(&args.relation, program)?;
    println!("passed");
    Ok(())
}

// = Sim command

#[derive(Args)]
struct SimArgs {
    #[command(flatten)]
    interpreter: InterpreterArgs,
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Target architecture: ebpf, psa, or v1model
    #[arg(long, value_name = "ARCH")]
    arch: String,
    /// Native plugin state encoding: arena-relative or arena-independent
    #[arg(long, default_value_t = Encoding::default(), value_name = "ENCODING")]
    plugin_encoding: Encoding,
    /// P4 program to simulate
    #[arg(short = 'p', value_name = "PROGRAM")]
    program: PathBuf,
    /// STF test to execute
    #[arg(long, value_name = "STF")]
    stf: PathBuf,
    /// Include directories for the P4 program
    #[arg(short = 'i', value_name = "DIR")]
    includes: Vec<PathBuf>,
    /// Disable interpreter call caching
    #[arg(long)]
    no_cache: bool,
    /// Check deterministic execution
    #[arg(long)]
    det: bool,
    /// Check interpreter guards
    #[arg(long)]
    guard: bool,
}

fn sim_command(args: SimArgs) -> Result<(), CliError> {
    let spec = interp_spec(&args.paths, &args.interpreter)?;
    let config = runner::Config::new(!args.no_cache, args.det, args.guard);
    let simulator = sim_plugin::build(spec, &args.arch, config, args.plugin_encoding)?;
    simulate(simulator, &args)
}

fn simulate(mut simulator: sim_plugin::Simulator, args: &SimArgs) -> Result<(), CliError> {
    simulator.run_stf_test(&args.includes, &args.program, &args.stf, |tx| {
        println!("[PASS] Transmitted {tx}");
    })?;
    println!("passed");
    Ok(())
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
    /// Structure specifications and print the representation without rule groups
    Struct(StructArgs),
    /// Convert specifications and print the prose representation
    Prose(ProseArgs),
    /// Run a P4 program with the algorithmic interpreter
    Run(RunArgs),
    /// Simulate a P4 program and STF test on a target architecture
    Sim(SimArgs),
}

fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Elab(args) => elab_command(args),
        Command::Algo(args) => algo_command(args),
        Command::Struct(args) => struct_command(args),
        Command::Prose(args) => prose_command(args),
        Command::Run(args) => run_command(args),
        Command::Sim(args) => sim_command(args),
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        // Successful commands have already written their output
        Ok(()) => ExitCode::SUCCESS,
        // Preserve source diagnostics from both parsing and elaboration
        Err(CliError::Spec(
            p4spec_rust::Error::Frontend(report) | p4spec_rust::Error::Elab(report),
        )) => {
            render_report(&report);
            ExitCode::FAILURE
        }
        // Report other typed failures once at the process boundary
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
