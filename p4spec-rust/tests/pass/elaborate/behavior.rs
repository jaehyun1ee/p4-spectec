use p4spec_rust::{lang::il::ast, pass::elaborate};

#[test]
fn test_function_clauses_are_populated_after_definition_traversal() {
    let spec_el = crate::spec_fixture::parse("dec $negate(bool) : bool\ndef $negate(true) = false")
        .expect("parse function declaration and clause");

    let spec_il = elaborate::convert(spec_el).expect("elaborate function");

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
    let spec_el = crate::spec_fixture::parse(
        "syntax pair<K, V> = K ':' V\n\
         syntax map<K, V> = pair<K, V>\n\
         dec $take<K, V>(map<K, V>) : bool\n\
         def $take<K, V>((K ':' V)) = true",
    )
    .expect("parse variant alias and clause");

    let spec_il = elaborate::convert(spec_el).expect("elaborate variant alias and clause");

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
fn test_matching_parameterized_forward_type_definition_is_accepted() {
    let spec_el = crate::spec_fixture::parse("syntax foo<T>\nsyntax foo<T> = nat")
        .expect("parse matching forward type definition");

    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_ok(), "{result:?}");
    assert!(warnings.is_empty());
}

#[test]
fn test_failed_variant_alternative_does_not_leak_wildcard_bindings() {
    let spec_el = crate::spec_fixture::parse(
        "syntax choice =\n\
         | bool BAD\n\
         | bool GOOD\n\
         dec $pick(choice) : bool\n\
         def $pick(_ GOOD) = true",
    )
    .expect("parse variant alternatives");

    let spec_il = elaborate::convert(spec_el).expect("elaborate matching alternative");
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
            .any(|exp| { matches!(&exp.node, ast::ExpKind::Id(id) if id.node == "_bool") })
    );
}

#[test]
fn test_zero_arity_default_input_hint_supports_positive_and_negated_premises() {
    let spec_el = crate::spec_fixture::parse(
        "relation R: _OK\n\
         rule R/base: _OK\n\
         relation S: _OK\n\
         rule S/base: _OK\n\
         -- R: _OK\n\
         -- R:/ _OK",
    )
    .expect("parse atom-only relations");

    let (result, warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(warnings.len(), 2);
}

#[test]
fn test_function_argument_signatures_are_alpha_equivalent() {
    let spec_el = crate::spec_fixture::parse(
        "dec $passed<T>(T) : T\n\
         dec $caller(def $expected<U>(U) : U) : nat\n\
         dec $main : nat\n\
         def $main = $caller(def $passed)",
    )
    .expect("parse alpha-renamed function signatures");

    let (result, _warnings) = elaborate::convert_with_warnings(spec_el);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn test_relation_negative_argument_keeps_unary_inference_and_upcast() {
    let spec_el = crate::spec_fixture::parse(
        "relation F: nat |- int : int\n\
         hint(input %0 %1 %2)\n\
         rule F/base: 1 |- 0 : -1",
    )
    .expect("parse negative notation argument");
    let spec_il = elaborate::convert(spec_el).expect("accept unary minus at int");
    let text = p4spec_rust::lang::traits::print::Print::to_string(&spec_il);
    assert!(text.contains("1 |- 0 as int : -1 as int"), "{text}");
}

#[test]
fn test_notation_type_mismatch_retries_the_next_variant() {
    let spec_el = crate::spec_fixture::parse(
        "syntax choice =\n\
         | TAG nat BAD\n\
         | TAG bool GOOD\n\
         dec $take(choice) : bool\n\
         def $take(TAG true GOOD) = true",
    )
    .expect("parse recoverable type mismatch");
    let spec_il = elaborate::convert(spec_el).expect("try the later boolean candidate");
    let text = p4spec_rust::lang::traits::print::Print::to_string(&spec_il);
    assert!(text.contains("TAG true GOOD"), "{text}");
}

#[test]
fn test_numeric_operators_keep_integer_candidate_and_operand_upcast() {
    for text in ["dec $f : int\ndef $f = 1 + (1 - 2)", "dec $f : bool\ndef $f = $(1 < (1 - 2))"] {
        let spec_el = crate::spec_fixture::parse(text).expect("parse mixed numeric operands");
        let spec_il = elaborate::convert(spec_el).expect("select the integer candidate");
        let ast::DefKind::MetaFunc(ast::MetaFuncDef::Defined(defined_func_il)) = &spec_il[0].node
        else {
            panic!("expected defined function");
        };
        let exp_il = &defined_func_il.clauses[0].node.exp;
        let (exp_l_il, exp_r_il) = match &exp_il.node {
            ast::ExpKind::Bin(_, ast::OpTyp::Int, exp_l_il, exp_r_il)
            | ast::ExpKind::Cmp(_, ast::OpTyp::Int, exp_l_il, exp_r_il) => (exp_l_il, exp_r_il),
            _ => panic!("expected integer operator, got {:?}", exp_il.node),
        };
        assert!(matches!(exp_l_il.node, ast::ExpKind::UpCast(_, _)));
        assert!(matches!(exp_r_il.node, ast::ExpKind::Bin(_, ast::OpTyp::Int, _, _)));
    }
}
