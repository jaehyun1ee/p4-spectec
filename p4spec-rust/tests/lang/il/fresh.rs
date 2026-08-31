use super::*;

fn sourced_span(name: &str) -> Span {
    Span::new(Position::new(name, 0, 0), Position::new(name, 0, 0))
}

fn sourced_id(name: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: sourced_span(name),
    }
}

fn sourced_typ() -> ast::Typ {
    p4spec_rust::phrase! {
        node: ast::TypKind::Bool,
        span: sourced_span("type"),
    }
}

fn sourced_names(names: &[&str]) -> IdSet {
    names.iter().map(|name| sourced_id(name)).collect()
}

fn id_from(name: &str, file: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0)),
    }
}

#[test]
fn test_fresh_uses_aliases_collisions_wildcards_and_dimensions_deterministically() {
    let at = Span::new(Position::new("fresh", 0, 0), Position::new("fresh", 0, 0));
    let ids = names(&["bool", "bool'", "bool_1"]);
    assert_eq!(
        fresh_impl::var_from_typ(&IdMap::new(), &ids, at.clone(), &typ())
            .id
            .node,
        "bool''"
    );
    let mut aliases = IdMap::new();
    aliases.insert(id("B"), typ());
    let alias = fresh_impl::var_from_typ(&aliases, &IdSet::new(), at.clone(), &typ());
    assert_eq!(
        (
            alias.id.node.as_str(),
            alias.typ.node.clone(),
            alias.iters.as_slice()
        ),
        ("B", ast::TypKind::Bool, &[] as &[ast::Iter])
    );
    aliases.insert(id("C"), typ());
    assert_eq!(
        fresh_impl::var_from_typ(&aliases, &IdSet::new(), at.clone(), &typ())
            .id
            .node,
        "bool"
    );
    assert_eq!(
        fresh_impl::var_from_typ_wildcard(&IdMap::new(), &IdSet::new(), at.clone(), &typ())
            .id
            .node,
        "_bool"
    );
    let nested = p4spec_rust::phrase! { node: ast::TypKind::Iter(
        Box::new(p4spec_rust::phrase! {
            node: ast::TypKind::Iter(Box::new(typ()), ast::Iter::Opt),
            span: at.clone(),
        }),
        ast::Iter::List,
    ), span: at.clone() };
    let nested_var = fresh_impl::var_from_typ(&IdMap::new(), &IdSet::new(), at.clone(), &nested);
    assert_eq!(nested_var.id.node, "bool");
    assert_eq!(nested_var.typ.node, ast::TypKind::Bool);
    assert_eq!(nested_var.iters, vec![ast::Iter::Opt, ast::Iter::List]);
}
#[test]
fn test_fresh_exact_edges_preserve_aliases_regions_and_full_dimension_shapes() {
    let at = Span::new(
        Position::new("requested", 0, 0),
        Position::new("requested", 0, 0),
    );
    let alias_typ = p4spec_rust::phrase! { node: ast::TypKind::Bool, span: Span::new(
        Position::new("alias_type", 0, 0),
        Position::new("alias_type", 0, 0),
    ) };
    let mut aliases = IdMap::new();
    aliases.insert(id("bool"), alias_typ.clone());
    let rejected = fresh_impl::var_from_typ(&aliases, &IdSet::new(), at.clone(), &typ());
    assert_eq!(rejected.id.node, "bool");
    assert_eq!(rejected.id.span, at);
    assert_eq!(rejected.typ.span, Span::default());
    aliases.clear();
    aliases.insert(id("Alias"), alias_typ.clone());
    let selected = fresh_impl::var_from_typ(
        &aliases,
        &IdSet::new(),
        Span::new(
            Position::new("requested_alias", 0, 0),
            Position::new("requested_alias", 0, 0),
        ),
        &typ(),
    );
    assert_eq!(selected.id.node, "Alias");
    assert_eq!(
        selected.id.span,
        Span::new(
            Position::new("requested_alias", 0, 0),
            Position::new("requested_alias", 0, 0)
        )
    );
    assert_eq!(selected.typ, alias_typ);
    assert!(selected.iters.is_empty());
    let collision_ids = names(&["_bool", "_bool'"]);
    let wildcard = fresh_impl::var_from_typ_wildcard(
        &IdMap::new(),
        &collision_ids,
        Span::new(
            Position::new("wildcard", 0, 0),
            Position::new("wildcard", 0, 0),
        ),
        &typ(),
    );
    assert_eq!(wildcard.id.node, "_bool''");
    assert_eq!(
        wildcard.id.span,
        Span::new(
            Position::new("wildcard", 0, 0),
            Position::new("wildcard", 0, 0)
        )
    );
    let iter_bool = p4spec_rust::phrase! { node: ast::TypKind::Iter(Box::new(typ()), ast::Iter::List), span: Span::new(
        Position::new("iter_type", 0, 0),
        Position::new("iter_type", 0, 0),
    ) };
    let inside_iter = fresh_impl::var_from_typ(
        &aliases,
        &IdSet::new(),
        Span::new(
            Position::new("inside_iter", 0, 0),
            Position::new("inside_iter", 0, 0),
        ),
        &iter_bool,
    );
    assert_eq!(inside_iter.id.node, "Alias");
    assert_eq!(
        inside_iter.id.span,
        Span::new(
            Position::new("inside_iter", 0, 0),
            Position::new("inside_iter", 0, 0)
        )
    );
    assert_eq!(
        inside_iter.typ,
        p4spec_rust::phrase! { node: ast::TypKind::Bool, span: Span::new(
            Position::new("alias_type", 0, 0),
            Position::new("alias_type", 0, 0)
        ) }
    );
    assert_eq!(inside_iter.iters, vec![ast::Iter::List]);
    let base_typ = p4spec_rust::phrase! { node: ast::TypKind::Bool, span: Span::new(
        Position::new("base_type", 0, 0),
        Position::new("base_type", 0, 0),
    ) };
    let nested = p4spec_rust::phrase! { node: ast::TypKind::Iter(
        Box::new(p4spec_rust::phrase! {
            node: ast::TypKind::Iter(Box::new(base_typ.clone()), ast::Iter::Opt),
            span: Span::new(
                Position::new("nested_inner", 0, 0),
                Position::new("nested_inner", 0, 0),
            ),
        }),
        ast::Iter::List,
    ), span: Span::new(
        Position::new("nested_type", 0, 0),
        Position::new("nested_type", 0, 0),
    ) };
    for dim in [false, true] {
        let (ids, expression) =
            fresh_impl::exp_from_typ(dim, &IdMap::new(), &IdSet::new(), &nested);
        assert_eq!(ids, names(&["bool"]));
        assert_iterated_exp(&expression, dim, &nested.span, &base_typ.span);
    }
}

#[test]
fn test_fresh_names_combine_aliases_collisions_wildcards_and_nested_dimensions() {
    let requested = sourced_span("requested");
    let alias_typ = p4spec_rust::phrase! {
        node: ast::TypKind::Bool,
        span: sourced_span("alias-type"),
    };
    let nested = p4spec_rust::phrase! { node: ast::TypKind::Iter(
        Box::new(p4spec_rust::phrase! {
            node:
            ast::TypKind::Iter(Box::new(sourced_typ()), ast::Iter::Opt),
            span: sourced_span("inner-iteration"),
        }),
        ast::Iter::List,
    ), span: sourced_span("outer-iteration") };
    let mut aliases = IdMap::new();
    aliases.insert(sourced_id("Alias"), alias_typ.clone());

    let variable = fresh_impl::var_from_typ(
        &aliases,
        &sourced_names(&["Alias", "Alias'", "Alias_1"]),
        requested.clone(),
        &nested,
    );
    assert_eq!(variable.id.node, "Alias''");
    assert_eq!(variable.id.span, requested);
    assert_eq!(variable.typ, alias_typ);
    assert_eq!(variable.iters, vec![ast::Iter::Opt, ast::Iter::List]);

    aliases.insert(
        sourced_id("Other"),
        p4spec_rust::phrase! {
            node: ast::TypKind::Bool,
            span: sourced_span("other-type"),
        },
    );
    let wildcard = fresh_impl::var_from_typ_wildcard(
        &aliases,
        &sourced_names(&["_bool", "_bool'", "_bool_1"]),
        requested.clone(),
        &nested,
    );
    assert_eq!(wildcard.id.node, "_bool''");
    assert_eq!(wildcard.id.span, requested);
    assert_eq!(wildcard.typ.node, ast::TypKind::Bool);
    assert_eq!(wildcard.iters, vec![ast::Iter::Opt, ast::Iter::List]);

    let (generated_ids, generated) =
        fresh_impl::exp_from_typ(true, &aliases, &sourced_names(&["bool"]), &nested);
    assert_eq!(generated_ids, sourced_names(&["bool", "bool'"]));
    let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_binders)) = generated.node else {
        panic!("outer iteration")
    };
    let ast::ExpKind::Iter(_, (ast::Iter::Opt, inner_binders)) = inner.node else {
        panic!("inner iteration")
    };
    assert_eq!(inner_binders.len(), 1);
    assert_eq!(outer_binders.len(), 1);
    assert!(inner_binders[0].iters.is_empty());
    assert_eq!(outer_binders[0].iters, vec![ast::Iter::Opt]);
}

#[test]
fn test_fresh_variables_lookup_aliases_by_identifier_text() {
    let span_requested = Span::new(
        Position::new("requested", 0, 0),
        Position::new("requested", 0, 0),
    );
    let typ = p4spec_rust::phrase! {
        node: ast::TypKind::Bool,
        span: Span::default(),
    };
    let mut metavars = IdMap::new();
    metavars.insert(id_from("Alias", "declaration"), typ.clone());

    let var = fresh_impl::var_from_typ(&metavars, &IdSet::new(), span_requested.clone(), &typ);

    assert_eq!(var.id.node, "Alias");
    assert_eq!(var.id.span, span_requested);
}
