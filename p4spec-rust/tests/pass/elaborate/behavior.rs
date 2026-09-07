use p4spec_rust::{frontend::parse::parse_string, lang::il::ast, pass::elaborate};

#[test]
fn test_function_clauses_are_populated_after_definition_traversal() {
    let spec = parse_string("dec $negate(bool) : bool\ndef $negate(true) = false")
        .expect("parse function declaration and clause");

    let spec = elaborate::elaborate(spec).expect("elaborate function");

    let ast::DefKind::FuncDec(function) = &spec[0].node else {
        panic!("expected function declaration");
    };
    assert_eq!(function.clauses.len(), 1);
    assert_eq!(function.clauses[0].span.left.line, 2);
}

#[test]
fn test_parenthesized_variant_keeps_the_case_origin() {
    let spec = parse_string(
        "syntax pair<K, V> = K ':' V\n\
         syntax map<K, V> = pair<K, V>\n\
         dec $take<K, V>(map<K, V>) : bool\n\
         def $take<K, V>((K ':' V)) = true",
    )
    .expect("parse variant alias and clause");

    let spec = elaborate::elaborate(spec).expect("elaborate variant alias and clause");

    let ast::DefKind::FuncDec(function) = &spec[2].node else {
        panic!("expected function declaration");
    };
    let ast::ArgKind::Exp(argument) = &function.clauses[0].node.args[0].node else {
        panic!("expected expression argument");
    };
    let ast::TypKind::Var(id, _) = argument.note.as_ref() else {
        panic!("expected nominal variant type");
    };
    assert_eq!(id.node, "pair");
    assert_eq!(id.span.left.line, 1);
}

#[test]
fn test_failed_variant_alternative_does_not_leak_wildcard_bindings() {
    let spec = parse_string(
        "syntax choice =\n\
         | bool BAD\n\
         | bool GOOD\n\
         dec $pick(choice) : bool\n\
         def $pick(_ GOOD) = true",
    )
    .expect("parse variant alternatives");

    let spec = elaborate::elaborate(spec).expect("elaborate matching alternative");
    let ast::DefKind::FuncDec(function) = &spec[1].node else {
        panic!("expected function declaration");
    };
    let ast::ArgKind::Exp(argument) = &function.clauses[0].node.args[0].node else {
        panic!("expected expression argument");
    };
    let ast::ExpKind::Case(case) = &argument.node else {
        panic!("expected variant case");
    };

    assert!(
        case.args()
            .iter()
            .any(|exp| { matches!(&exp.node, ast::ExpKind::Var(id) if id.node == "_bool") })
    );
}
