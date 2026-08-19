use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand};
use p4spec_rust::{
    interface::{BuiltinInterface, NullExtern},
    interp::{
        common::InterpError,
        sl::{Interpreter, Options},
    },
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
    },
};
use serde_json::Value;
use thiserror::Error;

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
    #[error("expected schema `{SL_SCHEMA}`, got `{0}`")]
    ExpectedSlSchema(String),
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
        BuiltinInterface::new(),
        NullExtern,
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

fn main() -> Result<(), CliError> {
    match Cli::parse().command {
        Command::Run(args) => run(args),
    }
}
