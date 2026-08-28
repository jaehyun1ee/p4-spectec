use p4spec_rust::{
    lang::{
        common::{
            ds::map::ArityMismatch,
            notation::mixfix::Mixfix,
            source::{Span, Spanned},
        },
        il::ast::{self, DefTypKind, FuncTyp, Iter, Subcheck, TypKind},
    },
    runtime::types::{
        TDEnv, Theta, TypeArityMismatch, TypeDef, TypeErrorKind, equiv_func_typ, equiv_not_typ,
        equiv_typ, expand_typ, optimize_sub_typ, sub_typ, subst_not_typ, subst_typ,
        typ as make_typ,
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

fn not_typ(name: &str) -> ast::NotTyp {
    Spanned::new(Mixfix::Arg(var(name, vec![])), Span::default())
}

fn func_typ(tparams: Vec<ast::TParam>, typs_params: Vec<ast::Typ>, typ_ret: ast::Typ) -> FuncTyp {
    FuncTyp {
        tparams,
        typs_params,
        typ_ret: Box::new(typ_ret),
    }
}

#[test]
fn type_iteration_wraps_in_order_and_preserves_the_inner_span() {
    let span = Span::new(
        p4spec_rust::lang::common::source::Position::new("type", 1, 2),
        p4spec_rust::lang::common::source::Position::new("type", 1, 6),
    );
    let typ_inner = Spanned::new(TypKind::Bool, span.clone());

    let typ = make_typ::iterate(typ_inner, &[Iter::Opt, Iter::List]);

    assert_eq!(typ.span, span);
    let TypKind::Iter(typ, Iter::List) = typ.node else {
        panic!("outer list type")
    };
    assert_eq!(typ.span, span);
    let TypKind::Iter(typ, Iter::Opt) = typ.node else {
        panic!("inner optional type")
    };
    assert_eq!(typ.span, span);
    assert_eq!(typ.node, TypKind::Bool);
}

#[test]
fn substitution_freshens_function_binders_and_rejects_higher_order_targets() {
    let mut theta = Theta::new();
    theta.insert(id("T"), typ(TypKind::Text));
    theta.insert(id("U"), typ(TypKind::Bool));

    let func_typ = func_typ(
        vec![id("T")],
        vec![var("T", vec![]), var("U", vec![])],
        var("T", vec![]),
    );
    let function = typ(TypKind::Func(func_typ));
    let substituted = subst_typ(&theta, &function).expect("substitute function");
    let TypKind::Func(func_typ) = substituted.node else {
        panic!("function type")
    };
    assert_eq!(func_typ.tparams[0].node, "__FRESH0");
    assert!(
        matches!(&func_typ.typs_params[0].node, TypKind::Var(id, args) if id.node == "__FRESH0" && args.is_empty())
    );
    assert_eq!(func_typ.typs_params[1].node, TypKind::Bool);
    assert!(
        matches!(&func_typ.typ_ret.node, TypKind::Var(id, args) if id.node == "__FRESH0" && args.is_empty())
    );

    let higher_order_span = Span::new(
        p4spec_rust::lang::common::source::Position::new("higher-order", 2, 3),
        p4spec_rust::lang::common::source::Position::new("higher-order", 2, 7),
    );
    let higher_order = Spanned::new(
        TypKind::Var(id("U"), vec![typ(TypKind::Bool)]),
        higher_order_span.clone(),
    );
    let error = subst_typ(&theta, &higher_order).unwrap_err();
    assert_eq!(error.kind, TypeErrorKind::HigherOrderSubstitution);
    assert_eq!(error.span, higher_order_span);
}

#[test]
fn public_substitutions_use_independent_fresh_type_identifiers() {
    let mut theta = Theta::new();
    theta.insert(id("X"), typ(TypKind::Bool));
    let func_typ = func_typ(vec![id("T")], vec![var("T", vec![])], var("T", vec![]));
    let function = typ(TypKind::Func(func_typ));

    let first = subst_typ(&theta, &function).expect("first substitution");
    let second = subst_typ(&theta, &function).expect("second substitution");
    let TypKind::Func(func_typ_first) = first.node else {
        panic!("first function type")
    };
    let TypKind::Func(func_typ_second) = second.node else {
        panic!("second function type")
    };
    assert_eq!(func_typ_first.tparams[0].node, "__FRESH0");
    assert_eq!(func_typ_second.tparams[0].node, "__FRESH0");
}

#[test]
fn substitution_maps_nested_notation_type_arguments() {
    let mut theta = Theta::new();
    theta.insert(id("T"), typ(TypKind::Bool));
    theta.insert(id("U"), typ(TypKind::Text));
    let not_typ = p4spec_rust::spanned!(
        node: Mixfix::Seq(vec![
            Mixfix::Arg(var("T", vec![])),
            Mixfix::Arg(var("U", vec![])),
        ]),
        span: Span::new(Default::default(), Default::default()),
    );

    let substituted = subst_not_typ(&theta, &not_typ).expect("substitute notation type");

    assert_eq!(
        substituted.node,
        Mixfix::Seq(vec![
            Mixfix::Arg(typ(TypKind::Bool)),
            Mixfix::Arg(typ(TypKind::Text)),
        ])
    );
    assert_eq!(substituted.span, not_typ.span);
}

#[test]
fn expansion_resolves_plain_aliases_and_reports_invalid_references() {
    let mut env = TDEnv::new();
    env.insert(
        id("Pair"),
        TypeDef::Defined(
            vec![id("T")],
            Box::new(plain(typ(TypKind::Tuple(vec![
                var("T", vec![]),
                var("T", vec![]),
            ])))),
        ),
    );

    let expanded = expand_typ(&env, &var("Pair", vec![typ(TypKind::Bool)]))
        .expect("expand parameterized alias");
    assert_eq!(
        expanded.node,
        TypKind::Tuple(vec![typ(TypKind::Bool), typ(TypKind::Bool)])
    );

    let arity = expand_typ(&env, &var("Pair", vec![])).unwrap_err();
    assert_eq!(
        arity.kind,
        TypeErrorKind::ArityMismatch(TypeArityMismatch::TypeArgument(ArityMismatch::new(1, 0)))
    );
    let missing = expand_typ(&env, &var("Missing", vec![])).unwrap_err();
    assert_eq!(
        missing.kind,
        TypeErrorKind::UndefinedType("Missing".to_owned())
    );
}

#[test]
fn equivalence_expands_aliases_and_alpha_renames_function_parameters() {
    let mut env = TDEnv::new();
    env.insert(
        id("Truth"),
        TypeDef::Defined(vec![], Box::new(plain(typ(TypKind::Bool)))),
    );
    assert!(equiv_typ(&env, &var("Truth", vec![]), &typ(TypKind::Bool)).unwrap());

    let func_typ_l = func_typ(vec![id("T")], vec![var("T", vec![])], var("T", vec![]));
    let func_typ_r = func_typ(vec![id("U")], vec![var("U", vec![])], var("U", vec![]));
    let equivalent = equiv_func_typ(&env, &Span::default(), &func_typ_l, &func_typ_r)
        .expect("compare function signatures");
    assert!(equivalent);

    let func_typ_l = func_typ(vec![id("T")], vec![], typ(TypKind::Bool));
    let func_typ_r = func_typ(vec![], vec![], typ(TypKind::Bool));
    let error = equiv_func_typ(&env, &Span::default(), &func_typ_l, &func_typ_r).unwrap_err();
    assert_eq!(
        error.kind,
        TypeErrorKind::ArityMismatch(TypeArityMismatch::TypeParameter(ArityMismatch::new(1, 0)))
    );
}

#[test]
fn notation_equivalence_compares_shape_and_type_arguments() {
    let mut env = TDEnv::new();
    env.insert(
        id("Truth"),
        TypeDef::Defined(vec![], Box::new(plain(typ(TypKind::Bool)))),
    );
    let not_typ_l = not_typ("Truth");
    let not_typ_r = Spanned::new(Mixfix::Arg(typ(TypKind::Bool)), Span::default());
    let not_typ_shape = Spanned::new(
        Mixfix::Seq(vec![Mixfix::Arg(typ(TypKind::Bool))]),
        Span::default(),
    );

    assert!(equiv_not_typ(&env, &not_typ_l, &not_typ_r).unwrap());
    assert!(!equiv_not_typ(&env, &not_typ_l, &not_typ_shape).unwrap());
}

#[test]
fn subtyping_covers_numeric_iteration_tuple_and_variant_rules() {
    let mut env = TDEnv::new();
    env.insert(id("A"), TypeDef::Extern);
    env.insert(id("B"), TypeDef::Extern);
    env.insert(
        id("Small"),
        TypeDef::Defined(vec![], Box::new(variant(vec![not_typ("A")]))),
    );
    env.insert(
        id("Large"),
        TypeDef::Defined(vec![], Box::new(variant(vec![not_typ("A"), not_typ("B")]))),
    );

    assert!(
        sub_typ(
            &env,
            &typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Nat)),
            &typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Int)),
        )
        .unwrap()
    );
    assert!(
        sub_typ(
            &env,
            &iter(typ(TypKind::Bool), Iter::Opt),
            &iter(typ(TypKind::Bool), Iter::List),
        )
        .unwrap()
    );
    assert!(
        sub_typ(
            &env,
            &typ(TypKind::Tuple(vec![var("Small", vec![])])),
            &typ(TypKind::Tuple(vec![var("Large", vec![])])),
        )
        .unwrap()
    );
    assert!(!sub_typ(&env, &var("Large", vec![]), &var("Small", vec![])).unwrap());
}

#[test]
fn subtype_optimization_emits_structural_checks_only_when_needed() {
    let env = TDEnv::new();
    let typ_source = typ(TypKind::Tuple(vec![
        typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Int)),
        iter(typ(TypKind::Text), Iter::List),
    ]));
    let typ_target = typ(TypKind::Tuple(vec![
        typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Nat)),
        iter(typ(TypKind::Bool), Iter::List),
    ]));

    assert_eq!(
        optimize_sub_typ(&env, &typ_source, &typ_target).unwrap(),
        Subcheck::Tuple(vec![
            Subcheck::Recurse(typ(TypKind::Num(p4spec_rust::lang::xl::num::Typ::Nat))),
            Subcheck::Iter(Iter::List, Box::new(Subcheck::Recurse(typ(TypKind::Bool)))),
        ])
    );
    assert_eq!(
        optimize_sub_typ(&env, &typ(TypKind::Bool), &typ(TypKind::Bool)).unwrap(),
        Subcheck::Skip
    );
}
