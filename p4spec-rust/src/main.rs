use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};

use p4spec_rust::{
    diagnostic::{RenderConfig, Renderer, Report},
    frontend::parse::parse_files,
    interface::p4::parse::parse_file,
    lang::{al, data::value::external::Encoding, il, sl, traits::print::Print},
    pass::{algo, elaborate, structure},
    runner::{self, BuiltinInterface, Interpreter, Runner},
    sim_plugin::{self, dummy::Dummy},
};

// = Helpers

fn elab(paths: Vec<PathBuf>) -> Result<il::ast::Spec, ExitCode> {
    let spec_el = parse_files(paths).map_err(frontend_error)?;
    elaborate::convert(spec_el).map_err(command_error)
}

fn algo(paths: Vec<PathBuf>) -> Result<al::ast::Spec, ExitCode> {
    let spec_il = elab(paths)?;
    algo::convert(spec_il).map_err(command_error)
}

fn structure(paths: Vec<PathBuf>) -> Result<sl::ast::Spec, ExitCode> {
    let spec_al = algo(paths)?;
    let without_rule_groups = true;
    structure::convert(spec_al, without_rule_groups).map_err(command_error)
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

// = Struct command

#[derive(Args)]
struct StructArgs {
    /// Specification files in processing order
    #[arg(required = true, value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn struct_command(args: StructArgs) -> ExitCode {
    let spec_sl = match structure(args.paths) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    println!("{}", Print::to_string(&spec_sl));
    ExitCode::SUCCESS
}

/// Renders frontend reports while preserving their structured payloads.
fn frontend_error(report: Box<Report>) -> ExitCode {
    let mut renderer = Renderer::new(RenderConfig::default());
    if let Err(error) = renderer.emit_stderr(&report) {
        eprintln!("{report}\ndiagnostic rendering failed: {error}");
    }
    ExitCode::FAILURE
}

/// Preserves legacy boundary output until D10 completes diagnostic transport.
fn command_error(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("{error}");
    ExitCode::FAILURE
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
}

fn interp_spec(
    paths: Vec<PathBuf>,
    interpreter: &InterpreterArgs,
) -> Result<runner::Spec, ExitCode> {
    let spec_al = algo(paths)?;
    if interpreter.al {
        Ok(runner::Spec::Al(spec_al))
    } else {
        let without_rule_groups = true;
        structure::convert(spec_al, without_rule_groups)
            .map(runner::Spec::Sl)
            .map_err(command_error)
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

fn run_command(mut args: RunArgs) -> ExitCode {
    let spec = match interp_spec(std::mem::take(&mut args.paths), &args.interpreter) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    let config = runner::Config::new(!args.no_cache, args.det, args.guard);
    match spec {
        runner::Spec::Al(spec) => {
            let runner = match runner::build_al(spec, config, Dummy) {
                Ok(runner) => runner,
                Err(error) => return command_error(error),
            };
            run_program(runner, &args)
        }
        runner::Spec::Sl(spec) => {
            let runner = match runner::build_sl(spec, config, Dummy) {
                Ok(runner) => runner,
                Err(error) => return command_error(error),
            };
            run_program(runner, &args)
        }
    }
}

fn run_program<Interp>(
    mut runner: Runner<Interp, BuiltinInterface, Dummy>,
    args: &RunArgs,
) -> ExitCode
where
    Interp: Interpreter<BuiltinInterface, Dummy>,
    Interp::Error: std::fmt::Display,
{
    let program = match parse_file(runner.arena_mut(), &args.includes, &args.program) {
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

fn sim_command(mut args: SimArgs) -> ExitCode {
    let spec = match interp_spec(std::mem::take(&mut args.paths), &args.interpreter) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    let config = runner::Config::new(!args.no_cache, args.det, args.guard);
    let simulator = match sim_plugin::build(spec, &args.arch, config, args.plugin_encoding) {
        Ok(simulator) => simulator,
        Err(error) => return command_error(error),
    };
    simulate(simulator, &args)
}

fn simulate(mut simulator: sim_plugin::Simulator, args: &SimArgs) -> ExitCode {
    match simulator.run_stf_test(&args.includes, &args.program, &args.stf, |tx| {
        println!("[PASS] Transmitted {tx}");
    }) {
        Ok(()) => {}
        Err(error) => return command_error(error),
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
    /// Structure specifications and print the representation without rule groups
    Struct(StructArgs),
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
        Command::Struct(args) => struct_command(args),
        Command::Run(args) => run_command(args),
        Command::Sim(args) => sim_command(args),
    }
}
