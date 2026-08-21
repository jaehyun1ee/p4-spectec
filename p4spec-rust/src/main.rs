use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use p4spec_rust::{
    interface::{P4Interface, PlaceholderExtern},
    interp::{
        common::InterpError,
        sl::{Interpreter, Options},
    },
    sim::{ebpf::Ebpf, runner::SuiteRunner},
    wire::{
        Envelope, SL_SCHEMA, WireError,
        ocaml::{
            DecodeError,
            lang::{
                il::{ValueEnvelopeCodec, ValueEnvelopeDecodeError, ValueEnvelopeEncodeError},
                sl::SpecCodec,
            },
        },
        runtime_value,
        sim_suite::{SimEntry, SimSuiteCodec, SimSuiteDecodeError},
    },
};
use serde_json::Value;
use thiserror::Error;

#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    ValueDecode(#[from] ValueEnvelopeDecodeError),
    #[error(transparent)]
    ValueEncode(#[from] ValueEnvelopeEncodeError),
    #[error(transparent)]
    Interpreter(#[from] InterpError),
    #[error(transparent)]
    SimSuite(#[from] SimSuiteDecodeError),
    #[error("expected schema `{SL_SCHEMA}`, got `{0}`")]
    ExpectedSlSchema(String),
    #[error("only eBPF simulation suites are supported, got `{0}`")]
    UnsupportedArchitecture(String),
}

#[derive(Parser)]
#[command(name = "p4spec-rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run an SL relation for one or more exported program values.
    Run(RunArgs),
    /// Run an exported P4/STF simulation suite.
    Sim(SimArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Versioned SL JSON envelope exported by p4spectec.
    #[arg(long)]
    spec: PathBuf,
    /// Relation to invoke with each program as its only input.
    #[arg(long)]
    relation: String,
    /// Enable the interpreter call caches.
    #[arg(long)]
    cache: bool,
    /// Reject ambiguous instruction results.
    #[arg(long)]
    deterministic: bool,
    /// Check public inputs and external outputs against SL types.
    #[arg(long)]
    guard: bool,
    /// Versioned value JSON envelopes exported by p4spectec.
    #[arg(required = true)]
    programs: Vec<PathBuf>,
}

#[derive(Args)]
struct SimArgs {
    /// Versioned SL JSON envelope exported by p4spectec.
    #[arg(long)]
    spec: PathBuf,
    /// Versioned simulation suite exported by p4spectec.
    #[arg(long)]
    suite: PathBuf,
    /// Enable the interpreter call caches.
    #[arg(long)]
    cache: bool,
    /// Check public inputs and external outputs against SL types.
    #[arg(long)]
    guard: bool,
}

fn decode_spec(path: &PathBuf) -> Result<p4spec_rust::lang::sl::ast::Spec, CliError> {
    let bytes = fs::read(path)?;
    let envelope = Envelope::<Value>::from_slice(&bytes)?;
    if envelope.schema() != SL_SCHEMA {
        return Err(CliError::ExpectedSlSchema(envelope.schema().to_owned()));
    }
    SpecCodec::decode(envelope.payload()).map_err(Into::into)
}

fn run(args: RunArgs) -> Result<(), CliError> {
    let spec = decode_spec(&args.spec)?;
    let mut interpreter = Interpreter::new(
        &spec,
        Options {
            cache: args.cache,
            deterministic: args.deterministic,
            guard: args.guard,
        },
        P4Interface::from_sl_spec(&spec),
        PlaceholderExtern::new(),
    )?;
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for program_path in args.programs {
        let bytes = fs::read(program_path)?;
        let program = runtime_value::to_runtime(&ValueEnvelopeCodec::decode(&bytes)?);
        for value in interpreter.eval_program(&args.relation, &program)? {
            output.write_all(&ValueEnvelopeCodec::encode(&runtime_value::to_canonical(
                &value,
            ))?)?;
            output.write_all(b"\n")?;
        }
    }
    Ok(())
}

fn run_sim(args: SimArgs) -> Result<(), CliError> {
    let spec = decode_spec(&args.spec)?;
    let suite = SimSuiteCodec::decode(&fs::read(&args.suite)?)?;
    if suite.arch != "ebpf" {
        return Err(CliError::UnsupportedArchitecture(suite.arch));
    }
    let mut interpreter = Interpreter::new(
        &spec,
        Options {
            cache: args.cache,
            deterministic: false,
            guard: args.guard,
        },
        P4Interface::from_sl_spec(&spec),
        Ebpf::new(),
    )?;
    let total = suite.entries.len();
    let mut excluded = 0;
    let mut failed = 0;
    let mut patched = 0;
    let mut patched_excluded = 0;
    let mut patched_failed = 0;
    let mut excluded_by_group = std::collections::BTreeMap::<String, usize>::new();
    println!("Running simulation test (ebpf) on {total} files\n");
    for entry in suite.entries {
        match entry {
            SimEntry::Exclude {
                p4_path,
                stf_path,
                patched: is_patched,
                group,
            } => {
                println!(
                    "\n>>> Running simulation test (ebpf) on {p4_path} with packet input {stf_path}"
                );
                println!("Excluding file: {stf_path}");
                excluded += 1;
                if is_patched {
                    patched += 1;
                    patched_excluded += 1;
                }
                if let Some(group) = group {
                    *excluded_by_group.entry(group).or_default() += 1;
                }
            }
            SimEntry::Run {
                p4_path,
                stf_path,
                patched: is_patched,
                program,
                stf,
            } => {
                println!(
                    "\n>>> Running simulation test (ebpf) on {p4_path} with packet input {stf_path}"
                );
                if is_patched {
                    patched += 1;
                }
                interpreter.clear();
                match SuiteRunner::<Ebpf>::run_case(&mut interpreter, &program, &stf) {
                    Ok(transmitted) => {
                        for packet in transmitted {
                            println!("[PASS] Transmitted {packet}");
                        }
                        println!("Run success: {stf_path}");
                    }
                    Err(error) => {
                        println!("Error on run: {stf_path}");
                        eprintln!("Error on run: {stf_path}\n{error}");
                        failed += 1;
                        if is_patched {
                            patched_failed += 1;
                        }
                    }
                }
            }
        }
    }
    print_stats(
        total,
        excluded,
        failed,
        patched,
        patched_excluded,
        patched_failed,
        &excluded_by_group,
    );
    Ok(())
}

fn print_stats(
    total: usize,
    excluded: usize,
    failed: usize,
    patched: usize,
    patched_excluded: usize,
    patched_failed: usize,
    excluded_by_group: &std::collections::BTreeMap<String, usize>,
) {
    let name = "Running simulation test (ebpf)";
    let passed = total - excluded - failed;
    let rate = |count, denominator| {
        if denominator == 0 {
            0.0
        } else {
            count as f64 / denominator as f64 * 100.0
        }
    };
    println!(
        "\n{name}: [EXCLUDE] {excluded}/{total} ({:.2}%) [PASS] {passed}/{total} ({:.2}%) [FAIL] {failed}/{total} ({:.2}%) [PATCH] {patched}/{total} ({:.2}%)",
        rate(excluded, total),
        rate(passed, total),
        rate(failed, total),
        rate(patched, total),
    );
    let patched_passed = patched - patched_excluded - patched_failed;
    println!(
        "\n{name}: [PATCH]: [EXCLUDE] {patched_excluded}/{patched} ({:.2}%) [PASS] {patched_passed}/{patched} ({:.2}%) [FAIL] {patched_failed}/{patched} ({:.2}%)",
        rate(patched_excluded, patched),
        rate(patched_passed, patched),
        rate(patched_failed, patched),
    );
    if !excluded_by_group.is_empty() {
        println!("\n{name} [EXCLUDE by subdir]:");
        for (group, count) in excluded_by_group {
            println!("  {group}: {count}");
        }
    }
}

fn main() -> Result<(), CliError> {
    match Cli::parse().command {
        Command::Run(args) => run(args),
        Command::Sim(args) => run_sim(args),
    }
}
