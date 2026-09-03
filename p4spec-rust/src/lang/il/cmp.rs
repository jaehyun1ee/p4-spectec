//! Syntax comparison for intermediate-language data

use std::cmp::Ordering;

use crate::lang::{traits::cmp::SyntaxCmp, xl::num};

use super::ast::*;

// == Syntax comparison

// - Types

fn typ_tag(typ: &TypKind) -> u8 {
    match typ {
        TypKind::Bool => 0,
        TypKind::Num(_) => 1,
        TypKind::Text => 2,
        TypKind::Var(_, _) => 3,
        TypKind::Tuple(_) => 4,
        TypKind::Iter(_, _) => 5,
        TypKind::Func(_) => 6,
    }
}

impl SyntaxCmp for TypKind {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (TypKind::Bool, TypKind::Bool) | (TypKind::Text, TypKind::Text) => Ordering::Equal,
            (TypKind::Num(num_typ_l), TypKind::Num(num_typ_r)) => {
                num::compare_typ(*num_typ_l, *num_typ_r)
            }
            (TypKind::Var(id_l, targs_l), TypKind::Var(id_r, targs_r)) => {
                let ordering = id_l.syntax_cmp(id_r);
                if ordering != Ordering::Equal {
                    return ordering;
                }
                targs_l.as_slice().syntax_cmp(targs_r)
            }
            (TypKind::Tuple(typs_l), TypKind::Tuple(typs_r)) => {
                typs_l.as_slice().syntax_cmp(typs_r)
            }
            (TypKind::Iter(typ_l, iter_l), TypKind::Iter(typ_r, iter_r)) => {
                let ordering = typ_l.syntax_cmp(typ_r);
                if ordering != Ordering::Equal {
                    return ordering;
                }
                iter_l.syntax_cmp(iter_r)
            }
            (TypKind::Func(func_typ_l), TypKind::Func(func_typ_r)) => {
                func_typ_l.syntax_cmp(func_typ_r)
            }
            _ => typ_tag(self).cmp(&typ_tag(other)),
        }
    }
}

impl SyntaxCmp for FuncTyp {
    fn syntax_cmp(&self, other: &Self) -> Ordering {
        let ordering = self.tparams.as_slice().syntax_cmp(&other.tparams);
        if ordering != Ordering::Equal {
            return ordering;
        }
        let ordering = self.typs_params.as_slice().syntax_cmp(&other.typs_params);
        if ordering != Ordering::Equal {
            return ordering;
        }
        self.typ_ret.syntax_cmp(&other.typ_ret)
    }
}
