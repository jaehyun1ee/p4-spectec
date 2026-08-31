//! Constructors for intermediate-language types

use crate::lang::{
    common::source::Span,
    il::ast::{self, TypKind},
    xl::num,
};
use crate::phrase;

/// Wraps a type in each iterator from innermost to outermost
pub fn iterate(mut typ: ast::Typ, iters: &[ast::Iter]) -> ast::Typ {
    for iter in iters {
        let span = typ.span.clone();
        let typ_inner = Box::new(typ);
        let typ_kind = TypKind::Iter(typ_inner, *iter);
        typ = phrase!(node: typ_kind, span: span);
    }
    typ
}

pub fn bool() -> ast::Typ {
    let typ_kind = TypKind::Bool;
    phrase!(node: typ_kind, span: Span::default())
}

pub fn nat() -> ast::Typ {
    let num_typ = num::Typ::Nat;
    num(num_typ)
}

pub fn int() -> ast::Typ {
    let num_typ = num::Typ::Int;
    num(num_typ)
}

pub fn num(num_typ: num::Typ) -> ast::Typ {
    let typ_kind = TypKind::Num(num_typ);
    phrase!(node: typ_kind, span: Span::default())
}

pub fn text() -> ast::Typ {
    let typ_kind = TypKind::Text;
    phrase!(node: typ_kind, span: Span::default())
}

pub fn var(id: ast::Id, targs: Vec<ast::Targ>) -> ast::Typ {
    let typ_kind = TypKind::Var(id, targs);
    phrase!(node: typ_kind, span: Span::default())
}

pub fn tuple(typs: Vec<ast::Typ>) -> ast::Typ {
    let typ_kind = TypKind::Tuple(typs);
    phrase!(node: typ_kind, span: Span::default())
}

pub fn iter(typ: ast::Typ, iter: ast::Iter) -> ast::Typ {
    let typ_inner = Box::new(typ);
    let typ_kind = TypKind::Iter(typ_inner, iter);
    phrase!(node: typ_kind, span: Span::default())
}

pub fn opt(typ: ast::Typ) -> ast::Typ {
    let iter = ast::Iter::Opt;
    self::iter(typ, iter)
}

pub fn list(typ: ast::Typ) -> ast::Typ {
    let iter = ast::Iter::List;
    self::iter(typ, iter)
}

pub fn func(tparams: Vec<ast::TParam>, typs_params: Vec<ast::Typ>, typ_ret: ast::Typ) -> ast::Typ {
    let typ_ret = Box::new(typ_ret);
    let func_typ = ast::FuncTyp {
        tparams,
        typs_params,
        typ_ret,
    };
    let typ_kind = TypKind::Func(func_typ);
    phrase!(node: typ_kind, span: Span::default())
}
