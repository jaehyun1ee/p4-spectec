use p4spec_rust::{
    lang::common::source::{Position, Span},
    lang::{
        common::{ds::set::IdSet, notation::atom::Atom as DomainAtom},
        el::ast::{self, BinOp, ExpKind},
        traits::{free::Free, print::Print},
    },
};

fn span(file: &str) -> Span {
    Span::new(Position::new(file, 0, 0), Position::new(file, 0, 0))
}

fn id(name: &str, file: &str) -> ast::Id {
    p4spec_rust::phrase! {
        node: name.to_owned(),
        span: span(file),
    }
}

fn ids(names: &[&str]) -> IdSet {
    names
        .iter()
        .map(|name| id(name, "expected.watsup"))
        .collect()
}

fn exp(kind: ExpKind, file: &str) -> ast::Exp {
    p4spec_rust::phrase! {
        node: kind,
        span: span(file),
    }
}

fn atom(source: &str) -> ast::Atom {
    p4spec_rust::phrase! {
        node: DomainAtom::Keyword(source.to_owned()),
        span: span("atom.watsup"),
    }
}

fn plain(kind: ast::PlainTypKind) -> ast::PlainTyp {
    p4spec_rust::phrase! {
        node: kind,
        span: span("type.watsup"),
    }
}

fn bool_typ() -> ast::PlainTyp {
    plain(ast::PlainTypKind::Bool)
}

fn param(kind: ast::ParamKind) -> ast::Param {
    p4spec_rust::phrase! {
        node: kind,
        span: span("param.watsup"),
    }
}

fn prem(kind: ast::PremKind) -> ast::Prem {
    p4spec_rust::phrase! {
        node: kind,
        span: span("prem.watsup"),
    }
}

fn definition(kind: ast::DefKind) -> ast::Def {
    p4spec_rust::phrase! {
        node: kind,
        span: span("def.watsup"),
    }
}

#[path = "el/free.rs"]
mod free;
#[path = "el/print.rs"]
mod print;
