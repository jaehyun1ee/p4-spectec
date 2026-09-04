//! Types shared by the intermediate language representations

use std::cmp::Ordering;

use crate::lang::{
    common::{
        Id, Iter, TId,
        source::{Phrase, Span},
    },
    traits::cmp::SyntaxCmp,
    xl::num,
};
use crate::phrase;

pub type Typ = Phrase<TypKind>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypKind {
    /// `bool`
    Bool,
    /// `numtyp`
    Num(num::Typ),
    /// `text`
    Text,
    /// `id (`<` list(targ, `,`) `>`)?`
    Var(Id, Vec<Typ>),
    /// `(` list(typ, `,`) `)`
    Tuple(Vec<Typ>),
    /// `typ iter`
    Iter(Box<Typ>, Iter),
    /// `<` list(tparam, `,`) `>` `(` list(typ, `,`) `)` `:` typ
    Func(FuncTyp),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuncTyp {
    pub tparams: Vec<TId>,
    pub typs_params: Vec<Typ>,
    pub typ_ret: Box<Typ>,
}

// == Comparison

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TypTag {
    Bool,
    Num,
    Text,
    Var,
    Tuple,
    Iter,
    Func,
}

impl TypKind {
    fn tag(&self) -> TypTag {
        match self {
            Self::Bool => TypTag::Bool,
            Self::Num(_) => TypTag::Num,
            Self::Text => TypTag::Text,
            Self::Var(_, _) => TypTag::Var,
            Self::Tuple(_) => TypTag::Tuple,
            Self::Iter(_, _) => TypTag::Iter,
            Self::Func(_) => TypTag::Func,
        }
    }
}

impl SyntaxCmp for TypKind {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Bool, Self::Bool) | (Self::Text, Self::Text) => Ordering::Equal,
            (Self::Num(num_typ_l), Self::Num(num_typ_r)) => {
                num::compare_typ(*num_typ_l, *num_typ_r)
            }
            (Self::Var(id_l, targs_l), Self::Var(id_r, targs_r)) => id_l
                .syntax_cmp(id_r)
                .then_with(|| targs_l.as_slice().syntax_cmp(targs_r)),
            (Self::Tuple(typs_l), Self::Tuple(typs_r)) => typs_l.as_slice().syntax_cmp(typs_r),
            (Self::Iter(typ_l, iter_l), Self::Iter(typ_r, iter_r)) => typ_l
                .syntax_cmp(typ_r)
                .then_with(|| iter_l.syntax_cmp(iter_r)),
            (Self::Func(func_typ_l), Self::Func(func_typ_r)) => func_typ_l.syntax_cmp(func_typ_r),
            _ => self.tag().cmp(&other.tag()),
        }
    }
}

impl SyntaxCmp for FuncTyp {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        self.tparams
            .as_slice()
            .syntax_cmp(&other.tparams)
            .then_with(|| self.typs_params.as_slice().syntax_cmp(&other.typs_params))
            .then_with(|| self.typ_ret.syntax_cmp(&other.typ_ret))
    }
}

// == Smart constructors

pub mod make {
    use super::*;

    /// Wraps a type in each iterator from innermost to outermost
    pub fn iterate(mut typ: Typ, iters: &[Iter]) -> Typ {
        for iter in iters {
            let span = typ.span.clone();
            let typ_inner = Box::new(typ);
            let typ_kind = TypKind::Iter(typ_inner, *iter);
            typ = phrase!(node: typ_kind, span: span);
        }
        typ
    }

    pub fn bool() -> Typ {
        let typ_kind = TypKind::Bool;
        phrase!(node: typ_kind, span: Span::default())
    }

    pub fn nat() -> Typ {
        let num_typ = num::Typ::Nat;
        num(num_typ)
    }

    pub fn int() -> Typ {
        let num_typ = num::Typ::Int;
        num(num_typ)
    }

    pub fn num(num_typ: num::Typ) -> Typ {
        let typ_kind = TypKind::Num(num_typ);
        phrase!(node: typ_kind, span: Span::default())
    }

    pub fn text() -> Typ {
        let typ_kind = TypKind::Text;
        phrase!(node: typ_kind, span: Span::default())
    }

    pub fn var(id: Id, targs: Vec<Typ>) -> Typ {
        let typ_kind = TypKind::Var(id, targs);
        phrase!(node: typ_kind, span: Span::default())
    }

    pub fn tuple(typs: Vec<Typ>) -> Typ {
        let typ_kind = TypKind::Tuple(typs);
        phrase!(node: typ_kind, span: Span::default())
    }

    pub fn iter(typ: Typ, iter: Iter) -> Typ {
        let typ_inner = Box::new(typ);
        let typ_kind = TypKind::Iter(typ_inner, iter);
        phrase!(node: typ_kind, span: Span::default())
    }

    pub fn opt(typ: Typ) -> Typ {
        let iter = Iter::Opt;
        self::iter(typ, iter)
    }

    pub fn list(typ: Typ) -> Typ {
        let iter = Iter::List;
        self::iter(typ, iter)
    }

    pub fn func(tparams: Vec<TId>, typs_params: Vec<Typ>, typ_ret: Typ) -> Typ {
        let typ_ret = Box::new(typ_ret);
        let func_typ = FuncTyp {
            tparams,
            typs_params,
            typ_ret,
        };
        let typ_kind = TypKind::Func(func_typ);
        phrase!(node: typ_kind, span: Span::default())
    }
}
