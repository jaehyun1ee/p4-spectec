use p4spec_rust::lang::data::value::ValueArena;
use std::path::Path;

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{
        AlInterp, Config,
        context::Global,
        error::{Error, ErrorKind, HostErrorKind},
    },
    lang::data::value::Value,
    pass::{algo, elaborate},
    runner::{BuiltinInterface, Extern, ExternError, Runner},
};

#[path = "core/mod.rs"]
mod core;
#[path = "dummy.rs"]
mod dummy;
#[path = "io.rs"]
mod io;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn runner<Exn: Extern>(external: Exn) -> Runner<AlInterp, BuiltinInterface, Exn> {
    runner_from_spec(&repo().join("spec"), external)
}

fn runner_from_spec<Exn: Extern>(
    spec: &Path,
    external: Exn,
) -> Runner<AlInterp, BuiltinInterface, Exn> {
    let spec_el = parse_files([spec]).expect("native specification parsing");
    let spec_il = elaborate::elaborate(spec_el).expect("native elaboration");
    let spec_al = algo::convert(spec_il).expect("native algorithmic conversion");
    let unparser = P4Unparser::from_al_spec(&spec_al);
    Runner::new(
        Global::load(spec_al).unwrap(),
        AlInterp::new(Config::new(true, false, false)),
        BuiltinInterface::new(unparser),
        external,
    )
}

fn has_extern_failure(error: &Error, expected: &str) -> bool {
    matches!(
        error.kind.as_ref(),
        ErrorKind::Host(HostErrorKind::Extern(ExternError::Failure(message)))
            if message == expected
    ) || error
        .children
        .iter()
        .any(|error| has_extern_failure(error, expected))
}

fn parse_program(arena: &mut ValueArena, path: &Path) -> Value {
    parse_file(arena, &[repo().join("p4c/p4include")], path).expect("native P4 parsing")
}

#[path = "hash.rs"]
mod hash;

#[path = "table.rs"]
mod table;

#[path = "ebpf/mod.rs"]
mod ebpf;

#[path = "psa/mod.rs"]
mod psa;
