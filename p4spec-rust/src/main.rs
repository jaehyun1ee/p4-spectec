use std::{path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::parse::parse_file,
    interp::{al::Config as AlConfig, sl::Config as SlConfig},
    lang::{al, data::value::external::Encoding, il, sl, traits::print::Print},
    pass::{algo, elaborate, structure},
    runner::{self, BuiltinInterface, Interpreter, Runner},
    sim_plugin::{self, dummy::Dummy, runner::Error as SimError},
    stf,
};

// = Helpers

fn elab(paths: Vec<PathBuf>) -> Result<il::ast::Spec, ExitCode> {
    let spec_el = parse_files(paths).map_err(command_error)?;
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
    let spec_al = match algo(std::mem::take(&mut args.paths)) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    if args.interpreter.al {
        let config = AlConfig::new(!args.no_cache, args.det, args.guard);
        let runner = match runner::build_al(spec_al, config, Dummy) {
            Ok(runner) => runner,
            Err(error) => return command_error(error),
        };
        run_program(runner, &args)
    } else {
        let config = SlConfig::new(!args.no_cache, args.det, args.guard);
        let runner = match runner::build_sl(spec_al, config, Dummy) {
            Ok(runner) => runner,
            Err(error) => return command_error(error),
        };
        run_program(runner, &args)
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
    let spec_al = match algo(std::mem::take(&mut args.paths)) {
        Ok(spec) => spec,
        Err(code) => return code,
    };
    if args.interpreter.al {
        let config = AlConfig::new(!args.no_cache, args.det, args.guard);
        let simulator = match sim_plugin::build_with_encoding(
            spec_al,
            &args.arch,
            config,
            args.plugin_encoding,
        ) {
            Ok(simulator) => simulator,
            Err(error) => return command_error(error),
        };
        simulate(simulator, &args)
    } else {
        let config = SlConfig::new(!args.no_cache, args.det, args.guard);
        let simulator = match sim_plugin::build_sl_with_encoding(
            spec_al,
            &args.arch,
            config,
            args.plugin_encoding,
        ) {
            Ok(simulator) => simulator,
            Err(error) => return command_error(error),
        };
        simulate(simulator, &args)
    }
}

fn simulate<Interp>(mut simulator: sim_plugin::Simulator<Interp>, args: &SimArgs) -> ExitCode
where
    Interp: sim_plugin::SimulatorInterpreter,
{
    let mut run = match simulator.init_pipe(&args.includes, &args.program) {
        Ok(run) => run,
        Err(error) => return command_error(error),
    };
    let stmts = match stf::parse::parse_file(&args.stf) {
        Ok(stmts) => stmts,
        Err(error) => return command_error(SimError::from(error)),
    };
    for stmt in &stmts {
        match simulator.run_stf_stmt(&mut run, stmt) {
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
