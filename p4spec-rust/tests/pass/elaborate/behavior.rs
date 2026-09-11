use std::path::Path;

use p4spec_rust::{
    frontend::parse::{parse_files, parse_string},
    lang::il::ast,
    pass::elaborate,
};

#[test]
fn test_function_clauses_are_populated_after_definition_traversal() {
    let spec_el = parse_string("dec $negate(bool) : bool\ndef $negate(true) = false")
        .expect("parse function declaration and clause");

    let spec_il = elaborate::elaborate(spec_el).expect("elaborate function");

    let ast::DefKind::MetaFunc(meta_func_def_il) = &spec_il[0].node else {
        panic!("expected function declaration");
    };
    let ast::MetaFuncDef::Defined(defined_func_il) = meta_func_def_il else {
        panic!("expected defined function");
    };
    assert_eq!(defined_func_il.clauses.len(), 1);
    assert_eq!(defined_func_il.clauses[0].span.left.line, 2);
}

#[test]
fn test_parenthesized_variant_keeps_the_case_origin() {
    let spec_el = parse_string(
        "syntax pair<K, V> = K ':' V\n\
         syntax map<K, V> = pair<K, V>\n\
         dec $take<K, V>(map<K, V>) : bool\n\
         def $take<K, V>((K ':' V)) = true",
    )
    .expect("parse variant alias and clause");

    let spec_il = elaborate::elaborate(spec_el).expect("elaborate variant alias and clause");

    let ast::DefKind::MetaFunc(meta_func_def_il) = &spec_il[2].node else {
        panic!("expected function declaration");
    };
    let ast::MetaFuncDef::Defined(defined_func_il) = meta_func_def_il else {
        panic!("expected defined function");
    };
    let ast::ArgKind::Exp(exp_arg) = &defined_func_il.clauses[0].node.args[0].node else {
        panic!("expected expression argument");
    };
    let ast::TypKind::Var(id, _) = exp_arg.note.as_ref() else {
        panic!("expected nominal variant type");
    };
    assert_eq!(id.node, "pair");
    assert_eq!(id.span.left.line, 1);
}

#[test]
fn test_failed_variant_alternative_does_not_leak_wildcard_bindings() {
    let spec_el = parse_string(
        "syntax choice =\n\
         | bool BAD\n\
         | bool GOOD\n\
         dec $pick(choice) : bool\n\
         def $pick(_ GOOD) = true",
    )
    .expect("parse variant alternatives");

    let spec_il = elaborate::elaborate(spec_el).expect("elaborate matching alternative");
    let ast::DefKind::MetaFunc(meta_func_def_il) = &spec_il[1].node else {
        panic!("expected function declaration");
    };
    let ast::MetaFuncDef::Defined(defined_func_il) = meta_func_def_il else {
        panic!("expected defined function");
    };
    let ast::ArgKind::Exp(exp_arg) = &defined_func_il.clauses[0].node.args[0].node else {
        panic!("expected expression argument");
    };
    let ast::ExpKind::Case(case) = &exp_arg.node else {
        panic!("expected variant case");
    };

    assert!(
        case.args()
            .iter()
            .any(|exp| { matches!(&exp.node, ast::ExpKind::Var(id) if id.node == "_bool") })
    );
}

#[test]
#[ignore = "runs the full specification corpus"]
fn test_full_spec_elaborates() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");
    let spec_path = std::env::var_os("P4SPEC_TEST_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo.join("spec"));
    let spec = parse_files([spec_path]).expect("parse the specification corpus");

    elaborate::elaborate(spec).expect("elaborate the specification corpus");
}
