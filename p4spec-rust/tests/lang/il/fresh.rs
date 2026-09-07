use p4spec_rust::lang::{
    common::{
        ds::{map::IdMap, set::IdSet},
        source::{Position, Span},
    },
    il::{ast, fresh as fresh_impl},
};

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
