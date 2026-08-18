use std::rc::Rc;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::il::ast::{self as il, DefTypKind, TypKind},
    runtime::{
        r#type::{envs::TypeDefMap, typ::make as make_type, typdef::TypeDef},
        value::{
            ValueKind, ValueRef, make,
            r#match::{self as value_match, FuncSignature, MatchError, SubCache},
        },
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn atom(name: &str, file: &str) -> il::Atom {
    Spanned::new(Atom::Keyword(name.to_owned()), span(file))
}

fn var(name: &str, type_args: Vec<il::Typ>, file: &str) -> il::Typ {
    Spanned::new(TypKind::VarT(id(name, file), type_args), span(file))
}

fn no_func(_: &str) -> Option<FuncSignature<'static>> {
    None
}

fn sub(type_defs: &TypeDefMap, typ: &il::Typ, value: &ValueRef) -> Result<bool, MatchError> {
    value_match::sub(&mut SubCache::new(), type_defs, &no_func, typ, value)
}

#[test]
fn primitive_tuple_and_iteration_membership_follow_ocaml_cases() {
    let type_defs = TypeDefMap::new();
    let bool_value = make::bool(true, span("bool"));
    let nat_value = make::nat(BigInt::from(3), span("nat"));
    let positive_int = make::int(BigInt::from(3), span("positive-int"));
    let negative_int = make::int(BigInt::from(-1), span("negative-int"));
    let text_value = make::text("text".to_owned(), span("text"));

    assert!(sub(&type_defs, &make_type::bool_type(), &bool_value).unwrap());
    assert!(!sub(&type_defs, &make_type::bool_type(), &text_value).unwrap());
    assert!(sub(&type_defs, &make_type::nat_type(), &nat_value).unwrap());
    assert!(sub(&type_defs, &make_type::nat_type(), &positive_int).unwrap());
    assert!(!sub(&type_defs, &make_type::nat_type(), &negative_int).unwrap());
    assert!(sub(&type_defs, &make_type::int_type(), &nat_value).unwrap());
    assert!(sub(&type_defs, &make_type::text_type(), &text_value).unwrap());

    let tuple_type = make_type::tuple_type(vec![make_type::bool_type(), make_type::int_type()]);
    let tuple_value = make::tuple(
        &tuple_type,
        vec![Rc::clone(&bool_value), Rc::clone(&nat_value)],
        span("tuple"),
    );
    assert!(sub(&type_defs, &tuple_type, &tuple_value).unwrap());
    assert!(
        !sub(
            &type_defs,
            &make_type::tuple_type(vec![make_type::bool_type()]),
            &tuple_value,
        )
        .unwrap()
    );

    let bool_opt = make_type::opt_type(make_type::bool_type());
    let some_bool = make::opt(&bool_opt, Some(Rc::clone(&bool_value)), span("some-bool"));
    let none_bool = make::opt(&bool_opt, None, span("none-bool"));
    assert!(sub(&type_defs, &bool_opt, &some_bool).unwrap());
    assert!(sub(&type_defs, &bool_opt, &none_bool).unwrap());
    assert!(sub(&type_defs, &bool_opt, &bool_value).unwrap());

    let bool_list = make_type::list_type(make_type::bool_type());
    let list_value = make::list(
        &bool_list,
        vec![Rc::clone(&bool_value), make::bool(false, span("false"))],
        span("list"),
    );
    assert!(sub(&type_defs, &bool_list, &list_value).unwrap());
    assert!(!sub(&type_defs, &bool_list, &some_bool).unwrap());
}

#[test]
fn defined_plain_extern_and_struct_types_match_instantiated_values() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Alias".to_owned(),
        TypeDef::Defined(
            vec![id("T", "alias")],
            Box::new(Spanned::new(
                DefTypKind::PlainT(var("T", Vec::new(), "alias-body")),
                span("alias"),
            )),
        ),
    );
    type_defs.insert("External".to_owned(), TypeDef::Extern);
    type_defs.insert(
        "Pair".to_owned(),
        TypeDef::Defined(
            vec![id("T", "pair")],
            Box::new(Spanned::new(
                DefTypKind::StructT(vec![
                    (
                        atom("first", "first-type"),
                        var("T", Vec::new(), "first-type"),
                    ),
                    (atom("flag", "flag-type"), make_type::bool_type()),
                ]),
                span("pair"),
            )),
        ),
    );

    let alias_bool = var("Alias", vec![make_type::bool_type()], "alias-use");
    assert!(
        sub(
            &type_defs,
            &alias_bool,
            &make::bool(true, span("alias-value")),
        )
        .unwrap()
    );

    let external_type = var("External", Vec::new(), "external-use");
    let external_value = make::external(
        &external_type,
        ExternalData::String("payload".to_owned()),
        span("external-value"),
    );
    assert!(sub(&type_defs, &external_type, &external_value).unwrap());
    assert!(
        !sub(
            &type_defs,
            &external_type,
            &make::text("payload".to_owned(), span("text")),
        )
        .unwrap()
    );

    let pair_bool = var("Pair", vec![make_type::bool_type()], "pair-use");
    let pair_value = make::structure(
        &pair_bool,
        vec![
            (
                atom("first", "first-value"),
                make::bool(true, span("first-value")),
            ),
            (
                atom("flag", "flag-value"),
                make::bool(false, span("flag-value")),
            ),
        ],
        span("pair-value"),
    );
    assert!(sub(&type_defs, &pair_bool, &pair_value).unwrap());

    let wrong_field = make::structure(
        &pair_bool,
        vec![
            (
                atom("second", "second-value"),
                make::bool(true, span("second-value")),
            ),
            (
                atom("flag", "flag-value"),
                make::bool(false, span("flag-value")),
            ),
        ],
        span("wrong-field"),
    );
    assert!(!sub(&type_defs, &pair_bool, &wrong_field).unwrap());
}

#[test]
fn defined_variant_matches_notation_shape_and_instantiated_arguments() {
    let payload_type = var("T", Vec::new(), "payload-type");
    let some_notation = Spanned::new(
        Mixfix::Seq(vec![
            Mixfix::Atom(atom("SOME", "some-type")),
            Mixfix::Arg(payload_type),
        ]),
        span("some-type"),
    );
    let none_notation = Spanned::new(Mixfix::Atom(atom("NONE", "none-type")), span("none-type"));
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Choice".to_owned(),
        TypeDef::Defined(
            vec![id("T", "choice")],
            Box::new(Spanned::new(
                DefTypKind::VariantT(vec![
                    (
                        some_notation,
                        Spanned::new((id("Some", "origin"), Vec::new()), span("origin")),
                        Vec::new(),
                    ),
                    (
                        none_notation,
                        Spanned::new((id("None", "origin"), Vec::new()), span("origin")),
                        Vec::new(),
                    ),
                ]),
                span("choice"),
            )),
        ),
    );

    let choice_bool = var("Choice", vec![make_type::bool_type()], "choice-use");
    let some_bool = make::case(
        &choice_bool,
        Mixfix::Seq(vec![
            Mixfix::Atom(atom("SOME", "some-value")),
            Mixfix::Arg(make::bool(true, span("payload"))),
        ]),
        span("some-value"),
    );
    let some_text = make::case(
        &choice_bool,
        Mixfix::Seq(vec![
            Mixfix::Atom(atom("SOME", "some-value")),
            Mixfix::Arg(make::text("bad".to_owned(), span("payload"))),
        ]),
        span("some-value"),
    );
    let unknown = make::case(
        &choice_bool,
        Mixfix::Atom(atom("UNKNOWN", "unknown")),
        span("unknown"),
    );

    assert!(sub(&type_defs, &choice_bool, &some_bool).unwrap());
    assert!(!sub(&type_defs, &choice_bool, &some_text).unwrap());
    assert!(!sub(&type_defs, &choice_bool, &unknown).unwrap());
}

#[test]
fn function_membership_uses_the_declared_function_signature() {
    let type_defs = TypeDefMap::new();
    let type_param = id("T", "expected");
    let expected_type = make_type::func_type(
        vec![type_param.clone()],
        vec![var("T", Vec::new(), "expected-param")],
        var("T", Vec::new(), "expected-return"),
    );
    let function_value = make::func(
        id("identity", "value"),
        vec![id("Ignored", "value")],
        vec![make_type::text_type()],
        make_type::text_type(),
        span("value"),
    );
    let actual_type_params = vec![id("U", "actual")];
    let actual_param_types = vec![var("U", Vec::new(), "actual-param")];
    let actual_return_type = var("U", Vec::new(), "actual-return");
    let find_func = |name: &str| {
        (name == "identity").then_some(FuncSignature {
            type_params: &actual_type_params,
            param_types: &actual_param_types,
            return_type: &actual_return_type,
        })
    };

    assert!(
        value_match::sub(
            &mut SubCache::new(),
            &type_defs,
            &find_func,
            &expected_type,
            &function_value,
        )
        .unwrap()
    );
    assert_eq!(
        value_match::sub(
            &mut SubCache::new(),
            &type_defs,
            &no_func,
            &expected_type,
            &function_value,
        ),
        Err(MatchError::UndefinedFunction {
            name: "identity".to_owned(),
            span: span("value"),
        })
    );
}

#[test]
fn malformed_defined_types_report_typed_errors() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert("Parameter".to_owned(), TypeDef::Param);
    type_defs.insert(
        "OneArg".to_owned(),
        TypeDef::Defined(
            vec![id("T", "definition")],
            Box::new(Spanned::new(
                DefTypKind::PlainT(var("T", Vec::new(), "body")),
                span("definition"),
            )),
        ),
    );

    let parameter = var("Parameter", Vec::new(), "parameter-use");
    assert_eq!(
        sub(&type_defs, &parameter, &make::bool(true, span("value"))),
        Err(MatchError::UnexpectedTypeVariable {
            span: span("parameter-use"),
        })
    );

    let missing = var("Missing", Vec::new(), "missing-use");
    assert_eq!(
        sub(&type_defs, &missing, &make::bool(true, span("value"))),
        Err(MatchError::UndefinedType {
            name: "Missing".to_owned(),
            span: span("missing-use"),
        })
    );

    let arity_error = var("OneArg", Vec::new(), "arity-use");
    assert_eq!(
        sub(&type_defs, &arity_error, &make::bool(true, span("value"))),
        Err(MatchError::TypeArgumentMismatch {
            expected: 1,
            actual: 0,
            span: span("arity-use"),
        })
    );
}

#[test]
fn simple_type_variable_cache_uses_semantic_value_keys_without_vid() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Alias".to_owned(),
        TypeDef::Defined(
            Vec::new(),
            Box::new(Spanned::new(
                DefTypKind::PlainT(make_type::bool_type()),
                span("alias"),
            )),
        ),
    );
    let alias = var("Alias", Vec::new(), "alias-use");
    let value_a = make::bool(true, span("value-a"));
    let value_b = make::new(ValueKind::BoolV(true), TypKind::TextT, span("value-b"));
    let mut cache = SubCache::new();

    assert!(value_match::sub(&mut cache, &type_defs, &no_func, &alias, &value_a).unwrap());
    assert!(value_match::sub(&mut cache, &type_defs, &no_func, &alias, &value_b).unwrap());
    assert_eq!(cache.len(), 1);

    let alias_with_arg = var("Alias", vec![make_type::bool_type()], "alias-arg-use");
    assert_eq!(
        value_match::sub(&mut cache, &type_defs, &no_func, &alias_with_arg, &value_a),
        Err(MatchError::TypeArgumentMismatch {
            expected: 0,
            actual: 1,
            span: span("alias-arg-use"),
        })
    );
    assert_eq!(cache.len(), 1);
}

#[test]
fn subs_requires_equal_lengths_and_checks_each_pair() {
    let type_defs = TypeDefMap::new();
    let types = vec![make_type::bool_type(), make_type::int_type()];
    let values = vec![
        make::bool(true, span("bool")),
        make::nat(BigInt::from(1), span("nat")),
    ];

    assert!(value_match::subs(&type_defs, &no_func, &types, &values).unwrap());
    assert!(!value_match::subs(&type_defs, &no_func, &types[..1], &values).unwrap());

    let wrong_values = vec![
        make::bool(true, span("bool")),
        make::text("wrong".to_owned(), span("wrong")),
    ];
    assert!(!value_match::subs(&type_defs, &no_func, &types, &wrong_values).unwrap());
}
