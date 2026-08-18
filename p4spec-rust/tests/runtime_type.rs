use p4spec_rust::{
    domain::{
        atom::Atom,
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{
        il::ast::{self as il, Iter, ParamKind, TypKind},
        sl::ast::{self as sl},
    },
    runtime::r#type::{
        envs::TypeDefMap,
        equiv::{self, EquivError},
        expand::{self, ExpandError},
        fresh,
        sub::{self, SubError},
        subst::{self, SubstError, TypeSubstitution},
        typ::make,
        typdef::TypeDef,
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str, file: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(file))
}

fn atom(node: Atom, file: &str) -> il::Atom {
    Spanned::new(node, span(file))
}

fn var(name: &str, file: &str) -> il::Typ {
    make::var_type(id(name, file), Vec::new())
}

fn var_with_args(name: &str, args: Vec<il::Typ>, file: &str) -> il::Typ {
    Spanned::new(TypKind::VarT(id(name, file), args), span(file))
}

fn defined_plain(type_params: Vec<il::TParam>, typ: il::Typ, file: &str) -> TypeDef {
    TypeDef::Defined(
        type_params,
        Box::new(Spanned::new(il::DefTypKind::PlainT(typ), span(file))),
    )
}

fn type_case(name: &str, typ: il::Typ, file: &str) -> il::TypCase {
    (
        Spanned::new(
            Mixfix::Seq(vec![
                Mixfix::Atom(atom(Atom::Keyword(name.to_owned()), file)),
                Mixfix::Arg(typ),
            ]),
            span(file),
        ),
        Spanned::new((id(name, file), Vec::new()), span(file)),
        Vec::new(),
    )
}

fn defined_variant(
    type_params: Vec<il::TParam>,
    type_cases: Vec<il::TypCase>,
    file: &str,
) -> TypeDef {
    TypeDef::Defined(
        type_params,
        Box::new(Spanned::new(
            il::DefTypKind::VariantT(type_cases),
            span(file),
        )),
    )
}

#[test]
fn type_constructors_and_parameter_conversion_follow_ocaml_order() {
    let iterated = make::iterate(make::bool_type(), &[Iter::Opt, Iter::List]);
    let TypKind::IterT(optional, Iter::List) = iterated.node else {
        panic!("expected outer list iterator");
    };
    assert!(matches!(optional.node, TypKind::IterT(_, Iter::Opt)));

    assert!(matches!(make::nat_type().node, TypKind::NumT(_)));
    assert!(matches!(make::int_type().node, TypKind::NumT(_)));
    assert!(matches!(make::text_type().node, TypKind::TextT));
    assert!(matches!(
        make::tuple_type(vec![make::bool_type()]).node,
        TypKind::TupleT(_)
    ));

    let param_il = Spanned::new(ParamKind::ExpP(make::text_type()), span("il"));
    assert!(matches!(make::of_param_il(&param_il).node, TypKind::TextT));

    let exp = il::Exp::new(il::ExpKind::BoolE(true), TypKind::BoolT, span("sl"));
    let param_sl = Spanned::new(sl::ParamKind::ExpP(make::bool_type(), exp), span("sl"));
    assert!(matches!(make::of_param_sl(&param_sl).node, TypKind::BoolT));
}

#[test]
fn type_definitions_use_one_string_keyed_hash_map() {
    let tparam = id("T", "definition");
    let defined = TypeDef::Defined(
        vec![tparam.clone()],
        Box::new(Spanned::new(
            il::DefTypKind::PlainT(make::bool_type()),
            span("definition"),
        )),
    );
    assert_eq!(defined.type_params(), std::slice::from_ref(&tparam));

    let mut type_defs = TypeDefMap::with_capacity(4);
    type_defs.insert(tparam.node.clone(), defined.clone());
    assert_eq!(type_defs.get("T"), Some(&defined));

    type_defs.insert("T".to_owned(), TypeDef::Extern);
    assert_eq!(type_defs.get("T"), Some(&TypeDef::Extern));
    assert_eq!(type_defs.len(), 1);
}

#[test]
fn freshening_is_deterministic_after_refresh() {
    fresh::refresh();
    let params = vec![id("T", "source"), id("U", "source")];
    let (theta, fresh_params) = subst::freshen_tparams(&params);

    assert_eq!(fresh_params[0].node, "__FRESH0");
    assert_eq!(fresh_params[1].node, "__FRESH1");
    assert!(matches!(
        theta.get("T").map(|typ| &typ.node),
        Some(TypKind::VarT(id, args)) if id.node == "__FRESH0" && args.is_empty()
    ));
}

#[test]
fn substitution_replaces_free_variables_and_freshens_binders() {
    fresh::refresh();
    let mut theta = TypeSubstitution::new();
    theta.insert("T".to_owned(), make::int_type());
    theta.insert("U".to_owned(), make::text_type());

    let function = make::func_type(
        vec![id("T", "binder")],
        vec![var("T", "bound-use"), var("U", "free-use")],
        var("T", "return"),
    );
    let substituted = subst::subst_type(&theta, &function).expect("substitute function type");
    let TypKind::FuncT(params, parameter_types, return_type) = substituted.node else {
        panic!("expected function type");
    };

    assert_eq!(params[0].node, "__FRESH0");
    assert!(matches!(
        parameter_types[0].node,
        TypKind::VarT(ref id, ref args) if id.node == "__FRESH0" && args.is_empty()
    ));
    assert!(matches!(parameter_types[1].node, TypKind::TextT));
    assert!(matches!(
        return_type.node,
        TypKind::VarT(ref id, ref args) if id.node == "__FRESH0" && args.is_empty()
    ));
}

#[test]
fn substitution_covers_variant_shapes_and_rejects_higher_order_use() {
    let mut theta = TypeSubstitution::new();
    theta.insert("T".to_owned(), make::bool_type());

    let not_type = Spanned::new(Mixfix::Arg(var("T", "notation")), span("notation"));
    let substituted = subst::subst_not_type(&theta, &not_type).expect("substitute notation");
    assert!(matches!(substituted.node, Mixfix::Arg(ref typ) if typ.node == TypKind::BoolT));

    let origin = Spanned::new(
        (
            id("Origin", "origin"),
            vec![Spanned::new(
                TypKind::VarT(id("T", "origin-arg"), Vec::new()),
                span("origin-arg"),
            )],
        ),
        span("origin"),
    );
    let case = (not_type, origin, Vec::new());
    let (_, origin, _) = subst::subst_type_case(&theta, &case).expect("substitute type case");
    assert!(matches!(origin.node.1[0].node, TypKind::BoolT));

    let higher_order = Spanned::new(
        TypKind::VarT(
            id("T", "higher-order"),
            vec![Spanned::new(TypKind::BoolT, span("argument"))],
        ),
        span("higher-order"),
    );
    assert!(matches!(
        subst::subst_type(&theta, &higher_order),
        Err(SubstError::HigherOrder { span }) if span == Region::for_file("higher-order")
    ));
}

#[test]
fn iterator_substitution_uses_the_substituted_child_region_like_ocaml() {
    let mut theta = TypeSubstitution::new();
    theta.insert(
        "T".to_owned(),
        Spanned::new(TypKind::BoolT, span("replacement")),
    );
    let iterated = Spanned::new(
        TypKind::IterT(Box::new(var("T", "child")), Iter::Opt),
        span("outer"),
    );

    let substituted = subst::subst_type(&theta, &iterated).expect("substitute iterator");
    assert_eq!(substituted.span, span("replacement"));
}

#[test]
fn expansion_substitutes_arguments_and_recursively_resolves_plain_aliases() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Pair".to_owned(),
        defined_plain(
            vec![id("A", "pair"), id("B", "pair")],
            make::tuple_type(vec![var("A", "pair-body"), var("B", "pair-body")]),
            "pair",
        ),
    );
    type_defs.insert(
        "Alias".to_owned(),
        defined_plain(
            vec![id("T", "alias")],
            var_with_args(
                "Pair",
                vec![var("T", "alias-body"), make::bool_type()],
                "alias-body",
            ),
            "alias",
        ),
    );

    let typ = var_with_args("Alias", vec![make::int_type()], "use-site");
    let expanded = expand::expand_type(&type_defs, &typ).expect("expand type alias");
    let TypKind::TupleT(types) = expanded.node else {
        panic!("expected expanded tuple type");
    };
    assert!(matches!(types[0].node, TypKind::NumT(_)));
    assert!(matches!(types[1].node, TypKind::BoolT));
}

#[test]
fn expansion_reports_undefined_types_and_plain_alias_arity_mismatches() {
    let undefined = var_with_args("Missing", Vec::new(), "undefined-use");
    assert_eq!(
        expand::expand_type(&TypeDefMap::new(), &undefined),
        Err(ExpandError::UndefinedType {
            name: "Missing".to_owned(),
            span: span("undefined-use"),
        })
    );

    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Alias".to_owned(),
        defined_plain(vec![id("T", "alias")], var("T", "alias-body"), "alias"),
    );
    let mismatch = var_with_args("Alias", Vec::new(), "arity-use");
    assert_eq!(
        expand::expand_type(&type_defs, &mismatch),
        Err(ExpandError::TypeArgumentMismatch {
            span: span("arity-use"),
        })
    );
}

#[test]
fn expansion_leaves_non_plain_type_definitions_unexpanded() {
    let cases = [
        ("Param", TypeDef::Param),
        ("Extern", TypeDef::Extern),
        ("Defining", TypeDef::Defining(vec![id("T", "defining")])),
        (
            "Struct",
            TypeDef::Defined(
                vec![id("T", "struct")],
                Box::new(Spanned::new(
                    il::DefTypKind::StructT(Vec::new()),
                    span("struct"),
                )),
            ),
        ),
        (
            "Variant",
            TypeDef::Defined(
                vec![id("T", "variant")],
                Box::new(Spanned::new(
                    il::DefTypKind::VariantT(Vec::new()),
                    span("variant"),
                )),
            ),
        ),
    ];
    let type_defs = cases
        .iter()
        .cloned()
        .map(|(name, type_def)| (name.to_owned(), type_def))
        .collect();

    for (name, _) in cases {
        let typ = var_with_args(name, Vec::new(), "use-site");
        assert_eq!(
            expand::expand_type(&type_defs, &typ).expect("retain non-plain type"),
            typ
        );
    }
}

#[test]
fn equivalence_expands_aliases_and_compares_each_type_shape() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Alias".to_owned(),
        defined_plain(Vec::new(), make::int_type(), "alias"),
    );
    type_defs.insert("Box".to_owned(), TypeDef::Param);

    let cases = [
        (make::bool_type(), make::bool_type(), true),
        (make::int_type(), make::int_type(), true),
        (make::text_type(), make::text_type(), true),
        (
            var_with_args("Box", vec![var("Alias", "alias-use")], "box"),
            var_with_args("Box", vec![make::int_type()], "box"),
            true,
        ),
        (
            make::tuple_type(vec![make::bool_type(), make::text_type()]),
            make::tuple_type(vec![make::bool_type(), make::text_type()]),
            true,
        ),
        (
            make::iter_type(make::bool_type(), Iter::Opt),
            make::iter_type(make::bool_type(), Iter::Opt),
            true,
        ),
        (make::bool_type(), make::text_type(), false),
        (make::nat_type(), make::int_type(), false),
        (
            var_with_args("Box", Vec::new(), "box"),
            var_with_args("Box", vec![make::int_type()], "box"),
            false,
        ),
        (
            make::tuple_type(vec![make::bool_type()]),
            make::tuple_type(vec![make::bool_type(), make::text_type()]),
            false,
        ),
        (
            make::iter_type(make::bool_type(), Iter::Opt),
            make::iter_type(make::bool_type(), Iter::List),
            false,
        ),
        (
            make::func_type(Vec::new(), Vec::new(), make::bool_type()),
            make::func_type(Vec::new(), Vec::new(), make::bool_type()),
            false,
        ),
    ];

    for (typ_a, typ_b, expected) in cases {
        assert_eq!(
            equiv::equiv_type(&type_defs, &typ_a, &typ_b).expect("compare types"),
            expected
        );
    }
}

#[test]
fn equivalence_propagates_type_expansion_errors() {
    let typ = var_with_args("Missing", Vec::new(), "missing-use");
    assert_eq!(
        equiv::equiv_type(&TypeDefMap::new(), &typ, &make::bool_type()),
        Err(EquivError::Expansion(ExpandError::UndefinedType {
            name: "Missing".to_owned(),
            span: span("missing-use"),
        }))
    );
}

#[test]
fn notation_equivalence_ignores_source_regions_but_compares_atoms_and_arguments() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Alias".to_owned(),
        defined_plain(Vec::new(), make::bool_type(), "alias"),
    );
    let notation_a = Spanned::new(
        Mixfix::Brack(
            atom(Atom::LParen, "left-a"),
            Box::new(Mixfix::Arg(var("Alias", "arg-a"))),
            atom(Atom::RParen, "right-a"),
        ),
        span("notation-a"),
    );
    let notation_b = Spanned::new(
        Mixfix::Brack(
            atom(Atom::LParen, "left-b"),
            Box::new(Mixfix::Arg(make::bool_type())),
            atom(Atom::RParen, "right-b"),
        ),
        span("notation-b"),
    );
    let notation_different = Spanned::new(
        Mixfix::Brack(
            atom(Atom::LBrack, "left-c"),
            Box::new(Mixfix::Arg(make::bool_type())),
            atom(Atom::RBrack, "right-c"),
        ),
        span("notation-c"),
    );

    assert!(
        equiv::equiv_not_type(&type_defs, &notation_a, &notation_b)
            .expect("compare equivalent notations")
    );
    assert!(
        !equiv::equiv_not_type(&type_defs, &notation_a, &notation_different)
            .expect("compare different notations")
    );
}

#[test]
fn function_equivalence_alpha_renames_type_parameters_through_aliases() {
    fresh::refresh();
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Wrap".to_owned(),
        defined_plain(vec![id("X", "wrap")], var("X", "wrap-body"), "wrap"),
    );

    assert!(
        equiv::equiv_func_type(
            &type_defs,
            &span("function"),
            &[id("T", "left")],
            &[var_with_args(
                "Wrap",
                vec![var("T", "left-param")],
                "left-param",
            )],
            &var("T", "left-return"),
            &[id("U", "right")],
            &[var("U", "right-param")],
            &var("U", "right-return"),
        )
        .expect("compare alpha-equivalent functions")
    );
}

#[test]
fn function_equivalence_reports_type_and_value_parameter_count_mismatches() {
    assert_eq!(
        equiv::equiv_func_type(
            &TypeDefMap::new(),
            &span("type-parameter-error"),
            &[id("T", "left")],
            &[],
            &make::bool_type(),
            &[],
            &[],
            &make::bool_type(),
        ),
        Err(EquivError::TypeParametersMismatch {
            span: span("type-parameter-error"),
        })
    );

    fresh::refresh();
    assert_eq!(
        equiv::equiv_func_type(
            &TypeDefMap::new(),
            &span("parameter-error"),
            &[id("T", "left")],
            &[var("T", "left-param")],
            &var("T", "left-return"),
            &[id("U", "right")],
            &[],
            &var("U", "right-return"),
        ),
        Err(EquivError::ParametersMismatch {
            span: span("parameter-error"),
        })
    );
}

#[test]
fn subtyping_uses_equivalence_then_numeric_widening() {
    let type_defs = TypeDefMap::new();
    let cases = [
        (make::bool_type(), make::bool_type(), true),
        (make::nat_type(), make::int_type(), true),
        (make::int_type(), make::nat_type(), false),
        (make::bool_type(), make::int_type(), false),
    ];

    for (typ_a, typ_b, expected) in cases {
        assert_eq!(
            sub::sub_type(&type_defs, &typ_a, &typ_b).expect("compare subtype"),
            expected
        );
    }
}

#[test]
fn tuple_subtyping_is_covariant_and_requires_equal_arity() {
    let type_defs = TypeDefMap::new();
    let tuple_a = make::tuple_type(vec![make::nat_type(), make::bool_type()]);
    let tuple_b = make::tuple_type(vec![make::int_type(), make::bool_type()]);
    let tuple_short = make::tuple_type(vec![make::int_type()]);

    assert!(sub::sub_type(&type_defs, &tuple_a, &tuple_b).expect("widen tuple elements"));
    assert!(!sub::sub_type(&type_defs, &tuple_b, &tuple_a).expect("reject narrowing tuple"));
    assert!(!sub::sub_type(&type_defs, &tuple_a, &tuple_short).expect("reject tuple arity"));
}

#[test]
fn iteration_subtyping_follows_optional_and_list_lifting_rules() {
    let type_defs = TypeDefMap::new();
    let cases = [
        (
            make::list_type(make::nat_type()),
            make::list_type(make::int_type()),
            true,
        ),
        (
            make::opt_type(make::nat_type()),
            make::list_type(make::int_type()),
            true,
        ),
        (make::nat_type(), make::opt_type(make::int_type()), true),
        (make::nat_type(), make::list_type(make::int_type()), true),
        (
            make::list_type(make::nat_type()),
            make::opt_type(make::int_type()),
            false,
        ),
        (make::opt_type(make::int_type()), make::int_type(), false),
    ];

    for (typ_a, typ_b, expected) in cases {
        assert_eq!(
            sub::sub_type(&type_defs, &typ_a, &typ_b).expect("compare iteration subtype"),
            expected
        );
    }
}

#[test]
fn variant_subtyping_uses_instantiated_notation_case_inclusion() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Small".to_owned(),
        defined_variant(
            vec![id("T", "small")],
            vec![type_case("Case", var("T", "small-case"), "small-case")],
            "small",
        ),
    );
    type_defs.insert(
        "Large".to_owned(),
        defined_variant(
            vec![id("U", "large")],
            vec![
                type_case("Case", var("U", "large-case"), "large-case"),
                type_case("Other", make::bool_type(), "large-other"),
            ],
            "large",
        ),
    );
    let small = var_with_args("Small", vec![make::int_type()], "small-use");
    let large = var_with_args("Large", vec![make::int_type()], "large-use");

    assert!(sub::sub_type(&type_defs, &small, &large).expect("small variant is included"));
    assert!(!sub::sub_type(&type_defs, &large, &small).expect("large variant is not included"));
}

#[test]
fn variant_subtyping_reports_type_argument_count_mismatches() {
    let mut type_defs = TypeDefMap::new();
    type_defs.insert(
        "Bad".to_owned(),
        defined_variant(
            vec![id("T", "bad")],
            vec![type_case("Case", var("T", "bad-case"), "bad-case")],
            "bad",
        ),
    );
    type_defs.insert(
        "Good".to_owned(),
        defined_variant(Vec::new(), Vec::new(), "good"),
    );
    let bad = var_with_args("Bad", Vec::new(), "bad-use");
    let good = var_with_args("Good", Vec::new(), "good-use");

    assert_eq!(
        sub::sub_type(&type_defs, &bad, &good),
        Err(SubError::TypeArgumentMismatch {
            span: span("bad-use"),
        })
    );
}
