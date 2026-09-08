use std::{path::Path, rc::Rc};

use p4spec_rust::{
    frontend::parse::parse_files,
    interface::p4::{parse::parse_file, unparse::P4Unparser},
    interp::al::{
        Al, Config,
        context::Global,
        error::{Error, ErrorKind, HostErrorKind},
    },
    lang::data::value::{Value, ValueArena},
    pass::{algo, elaborate},
    runner::{BuiltinInterface, Extern, ExternError, Runner},
};

#[path = "core/mod.rs"]
mod core;
#[path = "placeholder.rs"]
mod placeholder;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
}

fn runner<E: Extern>(externs: E) -> Runner<Al, BuiltinInterface, E> {
    runner_from_spec(&repo().join("spec"), externs)
}

fn runner_from_spec<E: Extern>(spec: &Path, externs: E) -> Runner<Al, BuiltinInterface, E> {
    let spec_el = parse_files([spec]).expect("native specification parsing");
    let spec_il = elaborate::elaborate(spec_el).expect("native elaboration");
    let spec_al = algo::convert(spec_il).expect("native algorithmic conversion");
    let unparser = P4Unparser::from_al_spec(&spec_al);
    Runner::new(
        Rc::new(Global::load(spec_al).unwrap()),
        Config::new(false, false),
        ValueArena::new(),
        BuiltinInterface::new(unparser),
        externs,
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
