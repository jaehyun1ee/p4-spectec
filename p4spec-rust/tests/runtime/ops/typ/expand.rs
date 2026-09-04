//! Type-expansion tests

use std::borrow::Cow;

use p4spec_rust::{
    lang::{
        common::source::Span,
        il::ast::{self, DefTypKind, TypKind},
    },
    phrase,
    runtime::{env::TDEnv, ops::typ::expand_typ, typdef::TypeDef},
};

fn id(name: &str) -> ast::Id {
    phrase!(node: name.to_owned(), span: Span::default())
}

fn typ(kind: TypKind) -> ast::Typ {
    phrase!(node: kind, span: Span::default())
}

#[test]
fn test_expand_typ_borrows_unchanged_types_and_owns_alias_expansions() {
    let mut tdenv = TDEnv::new();
    let typ_bool = typ(TypKind::Bool);
    let def_typ = phrase! {
        node: DefTypKind::Plain(typ_bool.clone()),
        span: Span::default(),
    };
    tdenv.insert(id("Truth"), TypeDef::Defined(vec![], Box::new(def_typ)));

    let expanded_bool = expand_typ(&tdenv, &typ_bool).expect("expand concrete type");
    assert!(matches!(
        expanded_bool,
        Cow::Borrowed(typ_expanded) if std::ptr::eq(typ_expanded, &typ_bool)
    ));

    let typ_alias = typ(TypKind::Var(id("Truth"), vec![]));
    let expanded_alias = expand_typ(&tdenv, &typ_alias).expect("expand type alias");
    assert!(matches!(
        expanded_alias,
        Cow::Owned(typ_expanded) if typ_expanded == typ_bool
    ));
}
