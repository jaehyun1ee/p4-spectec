use std::cmp::Ordering;

use crate::lang::{
    il::ast::{self, Iter, TypKind},
    traits::eq::SyntaxEq,
    xl::num,
};

fn type_tag(typ: &ast::Typ) -> u8 {
    match typ.node {
        TypKind::Bool => 0,
        TypKind::Num(_) => 1,
        TypKind::Text => 2,
        TypKind::Var(_, _) => 3,
        TypKind::Tuple(_) => 4,
        TypKind::Iter(_, _) => 5,
        TypKind::Func(_) => 6,
    }
}

fn compare_types(types_l: &[ast::Typ], types_r: &[ast::Typ]) -> Ordering {
    for (typ_l, typ_r) in types_l.iter().zip(types_r) {
        let ordering = compare_type(typ_l, typ_r);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    types_l.len().cmp(&types_r.len())
}

fn compare_type(typ_l: &ast::Typ, typ_r: &ast::Typ) -> Ordering {
    match (&typ_l.node, &typ_r.node) {
        (TypKind::Num(number_l), TypKind::Num(number_r)) => num::compare_typ(*number_l, *number_r),
        (TypKind::Var(id_l, args_l), TypKind::Var(id_r, args_r)) => id_l
            .node
            .cmp(&id_r.node)
            .then_with(|| compare_types(args_l, args_r)),
        (TypKind::Tuple(types_l), TypKind::Tuple(types_r)) => compare_types(types_l, types_r),
        (TypKind::Iter(typ_l, iter_l), TypKind::Iter(typ_r, iter_r)) => {
            compare_type(typ_l, typ_r).then_with(|| iter_l.cmp(iter_r))
        }
        _ => type_tag(typ_l).cmp(&type_tag(typ_r)),
    }
}

/// A base type paired with the iteration dimensions around it
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDimension {
    typ: ast::Typ,
    iters: Vec<Iter>,
}

impl TypeDimension {
    pub fn new(typ: ast::Typ, iters: Vec<Iter>) -> Self {
        Self { typ, iters }
    }

    pub fn typ(&self) -> &ast::Typ {
        &self.typ
    }

    pub fn iters(&self) -> &[Iter] {
        &self.iters
    }

    /// Compares type syntax without source spans, then iterator dimensions
    pub fn compare(&self, other: &Self) -> Ordering {
        compare_type(&self.typ, &other.typ).then_with(|| self.iters.cmp(&other.iters))
    }

    /// Tests type syntax and every dimension for equality
    pub fn equivalent(&self, other: &Self) -> bool {
        self.typ.syntax_eq(&other.typ) && self.iters == other.iters
    }

    /// Tests whether this value's dimensions are a prefix of `other`
    pub fn is_subdimension_of(&self, other: &Self) -> bool {
        self.typ.syntax_eq(&other.typ)
            && self.iters.len() <= other.iters.len()
            && self.iters == other.iters[..self.iters.len()]
    }

    /// Appends one outer iteration dimension
    pub fn with_iter(mut self, iter: Iter) -> Self {
        self.iters.push(iter);
        self
    }
}
