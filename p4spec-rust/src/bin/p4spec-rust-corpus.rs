use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

use clap::{Parser, ValueEnum};
use p4spec_rust::{
    interface::{P4Interface, PlaceholderExtern},
    interp::sl::{Interpreter, Options},
    lang::sl::ast::Spec,
    runtime::value::ValueRef,
    wire::{
        Envelope, SL_SCHEMA, WireError,
        ocaml::{
            DecodeError,
            lang::{
                il::{ValueEnvelopeCodec, ValueEnvelopeDecodeError},
                sl::SpecCodec,
            },
        },
        runtime_value,
    },
};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

const RELATION: &str = "Program_inst";

#[derive(Debug, Error)]
enum CorpusError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    ValueDecode(#[from] ValueEnvelopeDecodeError),
    #[error(transparent)]
    Interpreter(#[from] p4spec_rust::interp::common::InterpError),
    #[error("expected schema `{SL_SCHEMA}`, got `{0}`")]
    ExpectedSlSchema(String),
}

#[derive(Clone, Copy, ValueEnum)]
enum Expect {
    Pass,
    Fail,
    Any,
}

impl Expect {
    fn accepts(self, status: Status) -> bool {
        match self {
            Self::Pass => status == Status::Pass,
            Self::Fail => status == Status::Fail,
            Self::Any => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pass,
    Fail,
}

#[derive(Serialize)]
struct Record<'a> {
    program: &'a Path,
    relation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    iteration: Option<usize>,
    status: Status,
    decode_ns: u128,
    eval_ns: u128,
    output_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Parser)]
#[command(name = "p4spec-rust-corpus")]
struct Args {
    /// Versioned SL JSON envelope exported by p4spectec.
    #[arg(long)]
    spec: PathBuf,
    /// Expected Program_inst status for every program.
    #[arg(long, value_enum, default_value_t = Expect::Pass)]
    expect: Expect,
    /// Write result records to a JSONL file instead of stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Disable the interpreter call caches.
    #[arg(long)]
    no_cache: bool,
    /// Reject ambiguous instruction results.
    #[arg(long)]
    deterministic: bool,
    /// Check public inputs and external outputs against SL types.
    #[arg(long)]
    guard: bool,
    /// Warm-up evaluations per decoded program.
    #[arg(long, default_value_t = 0)]
    warmup: usize,
    /// Measured evaluations per decoded program.
    #[arg(long = "repeat", default_value = "1")]
    repeats: std::num::NonZeroUsize,
    /// Versioned value JSON envelopes exported by p4spectec.
    #[arg(required = true)]
    programs: Vec<PathBuf>,
}

fn decode_spec(path: &Path) -> Result<Spec, CorpusError> {
    let bytes = fs::read(path)?;
    let envelope = Envelope::<Value>::from_slice(&bytes)?;
    if envelope.schema() != SL_SCHEMA {
        return Err(CorpusError::ExpectedSlSchema(envelope.schema().to_owned()));
    }
    SpecCodec::decode(envelope.payload()).map_err(Into::into)
}

fn decode_program(path: &Path) -> Result<ValueRef, CorpusError> {
    let bytes = fs::read(path)?;
    Ok(runtime_value::to_runtime(&ValueEnvelopeCodec::decode(
        &bytes,
    )?))
}

fn run(args: Args) -> Result<ExitCode, CorpusError> {
    let spec_decode_start = Instant::now();
    let spec = decode_spec(&args.spec)?;
    let spec_decode_ns = spec_decode_start.elapsed().as_nanos();

    let interpreter_init_start = Instant::now();
    let mut interpreter = Interpreter::new(
        &spec,
        Options {
            cache: !args.no_cache,
            deterministic: args.deterministic,
            guard: args.guard,
        },
        P4Interface::from_sl_spec(&spec),
        PlaceholderExtern::new(),
    )?;
    let interpreter_init_ns = interpreter_init_start.elapsed().as_nanos();

    let mut output: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(fs::File::create(path)?),
        None => Box::new(io::stdout()),
    };
    let mut passes = 0usize;
    let mut failures = 0usize;
    let mut mismatches = 0usize;
    let mut eval_ns = 0u128;
    let repeats = args.repeats.get();

    for program_path in &args.programs {
        let decode_start = Instant::now();
        let program = decode_program(program_path)?;
        let decode_ns = decode_start.elapsed().as_nanos();
        for _ in 0..args.warmup {
            let _result = interpreter.eval_program(RELATION, &program);
        }
        for iteration in 1..=repeats {
            let eval_start = Instant::now();
            let result = interpreter.eval_program(RELATION, &program);
            let elapsed_ns = eval_start.elapsed().as_nanos();
            eval_ns += elapsed_ns;
            let record = match result {
                Ok(values) => {
                    passes += 1;
                    Record {
                        program: program_path,
                        relation: RELATION,
                        iteration: (repeats > 1).then_some(iteration),
                        status: Status::Pass,
                        decode_ns,
                        eval_ns: elapsed_ns,
                        output_count: Some(values.len()),
                        error: None,
                    }
                }
                Err(error) => {
                    failures += 1;
                    Record {
                        program: program_path,
                        relation: RELATION,
                        iteration: (repeats > 1).then_some(iteration),
                        status: Status::Fail,
                        decode_ns,
                        eval_ns: elapsed_ns,
                        output_count: None,
                        error: Some(error.to_string()),
                    }
                }
            };
            if !args.expect.accepts(record.status) {
                mismatches += 1;
            }
            serde_json::to_writer(&mut output, &record)?;
            output.write_all(b"\n")?;
        }
    }

    eprintln!(
        "programs={} evaluations={} pass={} fail={} mismatches={} spec_decode_ns={} \
         interpreter_init_ns={} eval_ns={}",
        args.programs.len(),
        args.programs.len() * repeats,
        passes,
        failures,
        mismatches,
        spec_decode_ns,
        interpreter_init_ns,
        eval_ns,
    );
    Ok(if mismatches == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn main() -> Result<ExitCode, CorpusError> {
    run(Args::parse())
}
