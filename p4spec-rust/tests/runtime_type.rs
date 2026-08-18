use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{
        il::ast::{self as il, Iter, ParamKind, TypKind},
        sl::ast::{self as sl},
    },
    runtime::r#type::{
        envs::TypeDefMap,
        fresh,
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

fn var(name: &str, file: &str) -> il::Typ {
    make::var_type(id(name, file), Vec::new())
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
