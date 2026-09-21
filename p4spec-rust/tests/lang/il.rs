use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::{
            ds::{map::IdMap, set::IdSet},
            notation::mixfix::Mixfix,
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

#[path = "il/free.rs"]
mod free;
#[path = "il/fresh.rs"]
mod fresh;
#[path = "il/has_call.rs"]
mod has_call;
#[path = "il/print.rs"]
mod print;
