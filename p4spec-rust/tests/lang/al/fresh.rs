use p4spec_rust::{
    lang::{
        al,
        common::{
            ds::set::IdSet,
            source::{Position, Span},
        },
        data::typ,
        il,
    },
    runtime::envs::algo::MEnv,
};

fn sourced_span(name: &str, line: i64) -> Span {
    let pos = Position::new(name, line, 0);
    Span::new(pos.clone(), pos)
}

fn sourced_id(name: &str, span: Span) -> il::ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span,
    }
}

#[test]
fn test_alias_expression_uses_stage_specific_span() {
    let span_decl = sourced_span("fresh.watsup", 3);
    let span_use = sourced_span("fresh.watsup", 9);
    let id_alias = sourced_id("flag", span_decl.clone());
    let mut typ_alias = typ::make::bool();
    typ_alias.span = span_decl.clone();
    let mut typ_use = typ::make::bool();
    typ_use.span = span_use.clone();
    let mut menv = MEnv::new();
    menv.insert(id_alias, typ_alias);

    let (_, exp_al) = al::fresh::exp_from_typ(true, &menv, &IdSet::new(), &typ_use);
    let (_, exp_il) = il::fresh::exp_from_typ(true, &menv, &IdSet::new(), &typ_use);

    assert_eq!(exp_al.span, span_decl);
    assert_eq!(exp_il.span, span_use);
}

#[test]
fn test_ambiguous_aliases_fall_back_and_avoid_collisions() {
    let span_use = sourced_span("use.watsup", 7);
    let mut typ_use = typ::make::bool();
    typ_use.span = span_use.clone();
    let mut menv = MEnv::new();
    menv.insert(
        sourced_id("flag", sourced_span("decl.watsup", 1)),
        typ::make::bool(),
    );
    menv.insert(
        sourced_id("condition", sourced_span("decl.watsup", 2)),
        typ::make::bool(),
    );
    let ids = ["bool", "bool'"]
        .into_iter()
        .map(|name| sourced_id(name, sourced_span("bound.watsup", 1)))
        .collect::<IdSet>();

    let (ids_fresh, exp_al) = al::fresh::exp_from_typ(false, &menv, &ids, &typ_use);
    let al::ast::ExpKind::Var(id_fresh) = exp_al.node else {
        panic!("fresh variable expression")
    };

    assert_eq!(id_fresh.node, "bool''");
    assert_eq!(id_fresh.span, span_use);
    assert!(ids_fresh.contains(&id_fresh));
    assert!(ids.iter().all(|id| ids_fresh.contains(id)));
}

#[test]
fn test_alias_dimensions_preserve_declaration_and_wrapper_spans() {
    let span_decl = sourced_span("decl.watsup", 4);
    let span_inner = sourced_span("use.watsup", 8);
    let span_outer = sourced_span("use.watsup", 9);
    let mut menv = MEnv::new();
    menv.insert(
        sourced_id("flag", span_decl.clone()),
        p4spec_rust::phrase! {
            node: al::ast::TypKind::Bool,
            span: span_decl.clone(),
        },
    );
    let typ_inner = p4spec_rust::phrase! {
        node: al::ast::TypKind::Iter(Box::new(typ::make::bool()), al::ast::Iter::Opt),
        span: span_inner.clone(),
    };
    let typ_outer = p4spec_rust::phrase! {
        node: al::ast::TypKind::Iter(Box::new(typ_inner), al::ast::Iter::List),
        span: span_outer,
    };

    let (_, exp_al) = al::fresh::exp_from_typ(true, &menv, &IdSet::new(), &typ_outer);
    let al::ast::ExpKind::Iter(exp_inner, (al::ast::Iter::List, vars_outer)) = exp_al.node else {
        panic!("outer iteration")
    };
    let al::ast::ExpKind::Iter(exp_base, (al::ast::Iter::Opt, vars_inner)) = exp_inner.node else {
        panic!("inner iteration")
    };
    let al::ast::ExpKind::Var(id_fresh) = exp_base.node else {
        panic!("fresh variable")
    };

    assert_eq!(id_fresh.span, span_decl);
    assert_eq!(exp_base.span, span_decl);
    assert_eq!(exp_inner.span, span_decl);
    assert_eq!(vars_inner.len(), 1);
    assert_eq!(vars_outer.len(), 1);
    assert_eq!(vars_inner[0].iters, Vec::<al::ast::Iter>::new());
    assert_eq!(vars_outer[0].iters, vec![al::ast::Iter::Opt]);
    assert_eq!(vars_inner[0].typ.span, span_decl);
    assert_eq!(vars_outer[0].typ.span, span_decl);
}
