use std::path::Path;

use p4spec_rust::{
    frontend::parse::{parse_files, parse_string},
    lang::il::ast,
    pass::elaborate,
};

#[test]
fn function_clauses_are_populated_after_definition_traversal() {
    let spec = parse_string("dec $negate(bool) : bool\ndef $negate(true) = false")
        .expect("parse function declaration and clause");

    let spec = elaborate::elaborate(&spec).expect("elaborate function");

    let ast::DefKind::FuncDec(function) = &spec[0].node else {
        panic!("expected function declaration");
    };
    assert_eq!(function.clauses.len(), 1);
    assert_eq!(function.clauses[0].span.left.line, 2);
}

#[test]
#[ignore = "runs the full specification corpus"]
fn full_spec_elaborates() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let spec_path = std::env::var_os("P4SPEC_TEST_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo.join("spec"));
    let spec = parse_files([spec_path]).expect("parse the specification corpus");

    elaborate::elaborate(&spec).expect("elaborate the specification corpus");
}
