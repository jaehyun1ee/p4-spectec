use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::{
            ds::{map::IdMap, set::IdSet},
            notation::{atom::Atom, mixfix::Mixfix},
        },
        hints::input::InputHint,
        il::{ast, fresh as fresh_impl},
        traits::print::Print,
    },
};

fn typ() -> ast::Typ {
    p4spec_rust::phrase! {
        node: ast::TypKind::Bool,
        span: Span::default(),
    }
}
fn id(name: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.into(),
        span: Span::default(),
    }
}
fn exp(kind: ast::ExpKind) -> ast::Exp {
    p4spec_rust::note_phrase! {
        node: kind,
        note: ast::TypKind::Bool,
        span: Span::default(),
    }
}
fn var(name: &str) -> ast::Exp {
    exp(ast::ExpKind::Var(id(name)))
}
fn arg(kind: ast::ArgKind) -> ast::Arg {
    p4spec_rust::phrase! {
        node: kind,
        span: Span::default(),
    }
}
fn prem(kind: ast::PremKind) -> ast::Prem {
    p4spec_rust::phrase! {
        node: kind,
        span: Span::default(),
    }
}
fn notexp(name: &str) -> ast::NotExp {
    Mixfix::Seq(vec![Mixfix::Arg(var(name))])
}
fn not_typ() -> ast::NotTyp {
    p4spec_rust::phrase! {
        node: Mixfix::Arg(typ()),
        span: Span::default(),
    }
}
fn names(names: &[&str]) -> IdSet {
    names.iter().map(|name| id(name)).collect()
}
fn hint() -> ast::Hint {
    (
        p4spec_rust::phrase! {
            node: "meta".into(),
            span: Span::default(),
        },
        p4spec_rust::phrase! { node: p4spec_rust::lang::el::ast::ExpKind::Var(
            p4spec_rust::phrase! {
                node: "payload".into(),
                span: Span::default(),
            },
        ), span: Span::default() },
    )
}

fn assert_iterated_exp(exp: &ast::Exp, dim: bool, id_span: &Span, typ_span: &Span) {
    let ast::ExpKind::Iter(inner, (ast::Iter::List, outer_binders)) = &exp.node else {
        panic!("outer iteration")
    };
    assert_eq!(&exp.span, id_span);
    let ast::TypKind::Iter(outer_typ, ast::Iter::List) = &exp.note else {
        panic!("outer type")
    };
    assert_eq!(&outer_typ.span, id_span);
    let ast::TypKind::Iter(base_typ, ast::Iter::Opt) = &outer_typ.node else {
        panic!("inner type")
    };
    assert_eq!(base_typ.node, ast::TypKind::Bool);
    assert_eq!(&base_typ.span, id_span);
    let ast::ExpKind::Iter(base, (ast::Iter::Opt, inner_binders)) = &inner.node else {
        panic!("inner iteration")
    };
    assert_eq!(&inner.span, id_span);
    assert!(
        matches!(&inner.note, ast::TypKind::Iter(typ, ast::Iter::Opt) if typ.node == ast::TypKind::Bool && typ.span == *id_span)
    );
    assert!(matches!(base.node, ast::ExpKind::Var(_)));
    assert_eq!(base.span, *id_span);
    assert_eq!(base.note, ast::TypKind::Bool);
    match (dim, inner_binders.as_slice(), outer_binders.as_slice()) {
        (false, [], []) => {}
        (true, [inner], [outer]) => {
            assert_eq!(&inner.id.span, id_span);
            assert_eq!(inner.id.node, "bool");
            assert_eq!(&inner.typ.span, typ_span);
            assert!(inner.iters.is_empty());
            assert!(
                matches!(inner.typ.node, ast::TypKind::Iter(ref typ, ast::Iter::Opt) if typ.node == ast::TypKind::Bool && typ.span == *id_span)
            );
            assert_eq!(&outer.id.span, id_span);
            assert_eq!(outer.id.node, "bool");
            assert_eq!(&outer.typ.span, typ_span);
            assert_eq!(outer.iters, vec![ast::Iter::Opt]);
            assert!(
                matches!(outer.typ.node, ast::TypKind::Iter(ref typ, ast::Iter::List) if matches!(typ.node, ast::TypKind::Iter(ref base, ast::Iter::Opt) if base.node == ast::TypKind::Bool && base.span == *id_span) && typ.span == *id_span)
            );
        }
        _ => panic!("binder shape"),
    }
}

#[path = "il/free.rs"]
mod free;
#[path = "il/fresh.rs"]
mod fresh;
#[path = "il/print.rs"]
mod print;
