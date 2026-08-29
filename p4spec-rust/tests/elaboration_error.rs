use std::path::Path;

use p4spec_rust::{
    frontend::parse::parse_files,
    lang::{
        common::source::{Position, Span, Spanned},
        il::ast::TypKind,
    },
    pass::elaborate::{self, ElabError, ElabErrorKind},
    runtime::types::{TDEnv, TypeErrorKind, expand_typ},
};

#[test]
fn runtime_type_failure_keeps_its_category_and_source_span() {
    let span = Span::new(
        Position::new("elaboration.watsup", 7, 2),
        Position::new("elaboration.watsup", 7, 8),
    );
    let id = Spanned::new("Missing".to_owned(), span.clone());
    let typ = Spanned::new(TypKind::Var(id, vec![]), span.clone());
    let type_error = expand_typ(&TDEnv::new(), &typ).unwrap_err();

    let error = ElabError::from(type_error);

    assert_eq!(
        error.kind,
        ElabErrorKind::Type(TypeErrorKind::UndefinedType("Missing".to_owned()))
    );
    assert_eq!(error.span, span);
}

#[test]
fn backtracking_failure_displays_its_elaboration_trace() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/elaboration/unmatched_variant.watsup");
    let spec = parse_files([fixture]).expect("parse unmatched variant fixture");

    let error = elaborate::elaborate(&spec).expect_err("reject unmatched variant fixture");
    let diagnostic = error.to_string();

    assert!(diagnostic.contains("expression elaboration failed"));
    assert!(diagnostic.contains("expression does not match any variant case"));
}
