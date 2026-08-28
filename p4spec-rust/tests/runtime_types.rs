use p4spec_rust::{
    lang::{
        common::{
            notation::mixfix::Mixfix,
            source::{Span, Spanned},
        },
        il::ast::{self, DefTypKind, Iter, ParamKind, Subcheck, TypKind},
    },
    runtime::types::{
        Substitution, TypeDefinition, TypeEnvironment, TypeErrorKind, equivalent_function_type,
        equivalent_type, expand_type, is_subtype, optimize_subtype, reset_fresh_type_ids,
        substitute_parameter, substitute_type,
    },
};

fn id(name: &str) -> ast::Id {
    Spanned::new(name.to_owned(), Span::default())
}

fn typ(kind: TypKind) -> ast::Typ {
    Spanned::new(kind, Span::default())
}

fn var(name: &str, args: Vec<ast::Targ>) -> ast::Typ {
    typ(TypKind::Var(id(name), args))
}

fn iter(inner: ast::Typ, iter: Iter) -> ast::Typ {
    typ(TypKind::Iter(Box::new(inner), iter))
}

fn plain(inner: ast::Typ) -> ast::DefTyp {
    Spanned::new(DefTypKind::Plain(inner), Span::default())
}

fn variant(cases: Vec<ast::NotTyp>) -> ast::DefTyp {
    let cases = cases
        .into_iter()
        .map(|not_typ| {
            (
                not_typ,
                Spanned::new((id("Origin"), vec![]), Span::default()),
                vec![],
            )
        })
        .collect();
    Spanned::new(DefTypKind::Variant(cases), Span::default())
}

fn notation_type(name: &str) -> ast::NotTyp {
    Spanned::new(Mixfix::Arg(var(name, vec![])), Span::default())
}

#[test]
fn substitution_freshens_function_binders_and_rejects_higher_order_targets() {
    reset_fresh_type_ids();
    let mut substitution = Substitution::new();
    substitution.insert(id("T"), typ(TypKind::Text));
    substitution.insert(id("U"), typ(TypKind::Bool));

    let function = typ(TypKind::Func(
        vec![id("T")],
        vec![var("T", vec![]), var("U", vec![])],
        Box::new(var("T", vec![])),
    ));
    let substituted = substitute_type(&substitution, &function).expect("substitute function");
    let TypKind::Func(tparams, params, result) = substituted.node else {
        panic!("function type")
    };
    assert_eq!(tparams[0].node, "__FRESH0");
    assert!(
        matches!(&params[0].node, TypKind::Var(id, args) if id.node == "__FRESH0" && args.is_empty())
    );
    assert_eq!(params[1].node, TypKind::Bool);
    assert!(
        matches!(&result.node, TypKind::Var(id, args) if id.node == "__FRESH0" && args.is_empty())
    );

    let higher_order_span = Span::new(
        p4spec_rust::lang::common::source::Position::new("higher-order", 2, 3),
        p4spec_rust::lang::common::source::Position::new("higher-order", 2, 7),
    );
    let higher_order = Spanned::new(
        TypKind::Var(id("U"), vec![typ(TypKind::Bool)]),
        higher_order_span.clone(),
    );
    let error = substitute_type(&substitution, &higher_order).unwrap_err();
    assert_eq!(error.kind, TypeErrorKind::HigherOrderSubstitution);
    assert_eq!(error.span, higher_order_span);
}

#[test]
fn public_substitutions_share_monotonic_fresh_type_identifiers() {
    reset_fresh_type_ids();
    let mut substitution = Substitution::new();
    substitution.insert(id("X"), typ(TypKind::Bool));
    let function = typ(TypKind::Func(
        vec![id("T")],
        vec![var("T", vec![])],
        Box::new(var("T", vec![])),
    ));

    let first = substitute_type(&substitution, &function).expect("first substitution");
    let second = substitute_type(&substitution, &function).expect("second substitution");
    let TypKind::Func(first_parameters, _, _) = first.node else {
        panic!("first function type")
    };
    let TypKind::Func(second_parameters, _, _) = second.node else {
        panic!("second function type")
    };
    assert_eq!(first_parameters[0].node, "__FRESH0");
    assert_eq!(second_parameters[0].node, "__FRESH1");
}

#[test]
fn substitution_preserves_parameter_binders_and_outer_spans() {
    reset_fresh_type_ids();
    let mut substitution = Substitution::new();
    substitution.insert(id("U"), typ(TypKind::Text));
    let parameter = Spanned::new(
        ParamKind::Def(
            id("callback"),
            vec![id("U")],
            vec![Spanned::new(
                ParamKind::Exp(var("U", vec![])),
                Span::default(),
            )],
            var("U", vec![]),
        ),
        Span::new(Default::default(), Default::default()),
    );

    let substituted =
        substitute_parameter(&substitution, &parameter).expect("substitute parameter");
    let ParamKind::Def(_, tparams, params, result) = substituted.node else {
        panic!("definition parameter")
    };
    assert_eq!(tparams[0].node, "__FRESH0");
    assert!(
        matches!(&params[0].node, ParamKind::Exp(typ) if matches!(&typ.node, TypKind::Var(id, _) if id.node == "__FRESH0"))
    );
    assert!(matches!(&result.node, TypKind::Var(id, _) if id.node == "__FRESH0"));
    assert_eq!(substituted.span, parameter.span);
}

#[test]
fn expansion_resolves_plain_aliases_and_reports_invalid_references() {
    let mut env = TypeEnvironment::new();
    env.insert(
        id("Pair"),
        TypeDefinition::Defined(
            vec![id("T")],
            Box::new(plain(typ(TypKind::Tuple(vec![
                var("T", vec![]),
                var("T", vec![]),
            ])))),
        ),
    );

    let expanded = expand_type(&env, &var("Pair", vec![typ(TypKind::Bool)]))
        .expect("expand parameterized alias");
    assert_eq!(
        expanded.node,
        TypKind::Tuple(vec![typ(TypKind::Bool), typ(TypKind::Bool)])
    );

    let arity = expand_type(&env, &var("Pair", vec![])).unwrap_err();
    assert_eq!(
        arity.kind,
        TypeErrorKind::TypeArgumentCount {
            expected: 1,
            actual: 0,
        }
    );
    let missing = expand_type(&env, &var("Missing", vec![])).unwrap_err();
    assert_eq!(
        missing.kind,
        TypeErrorKind::UndefinedType("Missing".to_owned())
    );
}

#[test]
fn equivalence_expands_aliases_and_alpha_renames_function_parameters() {
    let mut env = TypeEnvironment::new();
    env.insert(
        id("Truth"),
        TypeDefinition::Defined(vec![], Box::new(plain(typ(TypKind::Bool)))),
    );
    assert!(equivalent_type(&env, &var("Truth", vec![]), &typ(TypKind::Bool)).unwrap());

    let equivalent = equivalent_function_type(
        &env,
        &Span::default(),
        &[id("T")],
        &[var("T", vec![])],
        &var("T", vec![]),
        &[id("U")],
        &[var("U", vec![])],
        &var("U", vec![]),
    )
    .expect("compare function signatures");
    assert!(equivalent);

    let error = equivalent_function_type(
        &env,
        &Span::default(),
        &[id("T")],
        &[],
        &typ(TypKind::Bool),
        &[],
        &[],
        &typ(TypKind::Bool),
    )
    .unwrap_err();
    assert_eq!(
        error.kind,
        TypeErrorKind::TypeParameterCount { left: 1, right: 0 }
    );
}

#[test]
fn subtyping_covers_numeric_iteration_tuple_and_variant_rules() {
    let mut env = TypeEnvironment::new();
    env.insert(id("A"), TypeDefinition::Extern);
    env.insert(id("B"), TypeDefinition::Extern);
    env.insert(
        id("Small"),
        TypeDefinition::Defined(vec![], Box::new(variant(vec![notation_type("A")]))),
    );
    env.insert(
        id("Large"),
        TypeDefinition::Defined(
            vec![],
            Box::new(variant(vec![notation_type("A"), notation_type("B")])),
        ),
    );

    assert!(
        is_subtype(
            &env,
            &typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Nat)),
            &typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Int)),
        )
        .unwrap()
    );
    assert!(
        is_subtype(
            &env,
            &iter(typ(TypKind::Bool), Iter::Opt),
            &iter(typ(TypKind::Bool), Iter::List),
        )
        .unwrap()
    );
    assert!(
        is_subtype(
            &env,
            &typ(TypKind::Tuple(vec![var("Small", vec![])])),
            &typ(TypKind::Tuple(vec![var("Large", vec![])])),
        )
        .unwrap()
    );
    assert!(!is_subtype(&env, &var("Large", vec![]), &var("Small", vec![])).unwrap());
}

#[test]
fn subtype_optimization_emits_structural_checks_only_when_needed() {
    let env = TypeEnvironment::new();
    let source = typ(TypKind::Tuple(vec![
        typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Int)),
        iter(typ(TypKind::Text), Iter::List),
    ]));
    let target = typ(TypKind::Tuple(vec![
        typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Nat)),
        iter(typ(TypKind::Bool), Iter::List),
    ]));

    assert_eq!(
        optimize_subtype(&env, &source, &target).unwrap(),
        Subcheck::Tuple(vec![
            Subcheck::Recurse(typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Nat))),
            Subcheck::Iter(Iter::List, Box::new(Subcheck::Recurse(typ(TypKind::Bool)))),
        ])
    );
    assert_eq!(
        optimize_subtype(&env, &typ(TypKind::Bool), &typ(TypKind::Bool)).unwrap(),
        Subcheck::Skip
    );
}
