//! Slot instantiation of shared IL syntax
//!
//! `Prepared` is the IL stage with frame slots for identifiers and variables;
//! the type aliases name its forms, and the `Prepare` impls rewrite each node.
//! Iterations also register the outer variable `x*` for every iterated `x`,
//! so `eval::iter` can find its slot.

use std::rc::Rc;

use crate::lang::data::var::{IdSlot, VarSlot};

use crate::lang::il::ast::{self as source, Stage};

pub use crate::lang::il::ast::{
    Atom, BinOp, CmpOp, DefTyp, DefTypKind, DefinedTyp, ExternTyp, FuncTyp, Hint, Id, Iter,
    ListPattern, Mixop, NotTyp, NotTypKind, Num, NumOp, OpTyp, OptPattern, Param, ParamKind,
    TParam, Targ, TargKind, Text, Typ, TypCase, TypDef, TypField, TypKind, TypOrigin,
    TypOriginKind, UnOp, Value, ValueCase, ValueField, ValueKind, VarDef,
};

use crate::runtime::envs::interp::shared::frame::FrameLayout;

use super::Prepare;

// == Prepared syntax

// - Stage

/// IL syntax whose identifiers and variables are resolved to frame slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prepared;

impl Stage for Prepared {
    type Id = IdSlot;
    type Var = VarSlot;
    type Mixop = Rc<Mixop>;
}

// - Variables

/// A variable with its frame slot.
pub type Var = VarSlot;

// - Expressions

/// An expression over slot-resolved identifiers.
pub type Exp = source::Exp<Prepared>;
pub type ExpField = source::ExpField<Prepared>;
pub type ExpKind = source::ExpKind<Prepared>;
pub type NotExp = source::NotExp<Prepared>;
pub type ExpIter = source::ExpIter<VarSlot>;

// - Paths

/// A path over slot-resolved identifiers.
pub type Path = source::Path<Prepared>;
pub type PathKind = source::PathKind<Prepared>;

// - Arguments

/// An argument over slot-resolved identifiers.
pub type Arg = source::Arg<Prepared>;
pub type ArgKind = source::ArgKind<Prepared>;

// - Patterns and subtype checks

/// A pattern over prepared mixops.
pub type Pattern = source::Pattern<Prepared>;
/// A runtime subtype check over prepared mixops.
pub type Subcheck = source::Subcheck<Prepared>;

// - Premises

/// A premise iterator over slot-resolved variables.
pub type PremIter = source::PremIter<VarSlot>;

// == Preparation

// - Patterns and subtype checks

impl Prepare for source::Pattern {
    type Output = Pattern;

    fn prepare(self, _layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::Pattern::Case(mixop) => Pattern::Case(mixop),
            source::Pattern::List(pattern) => Pattern::List(pattern),
            source::Pattern::Opt(pattern) => Pattern::Opt(pattern),
        }
    }
}

impl Prepare for source::Subcheck {
    type Output = Subcheck;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::Subcheck::Skip => Subcheck::Skip,
            source::Subcheck::Mixop(mixops) => Subcheck::Mixop(mixops),
            source::Subcheck::Tuple(subchecks) => Subcheck::Tuple(subchecks.prepare(layout)),
            source::Subcheck::Iter(iter, subcheck) => {
                Subcheck::Iter(iter, subcheck.prepare(layout))
            }
            source::Subcheck::Recurse(typ) => Subcheck::Recurse(typ),
        }
    }
}
// - Expressions

impl Prepare for source::ExpKind {
    type Output = ExpKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::ExpKind::Bool(value_inner) => ExpKind::Bool(value_inner),
            source::ExpKind::Num(num_inner) => ExpKind::Num(num_inner),
            source::ExpKind::Text(text_inner) => ExpKind::Text(text_inner),
            source::ExpKind::Id(id_inner) => ExpKind::Id(id_inner.prepare(layout)),
            source::ExpKind::Un(op_inner, typ_op_inner, exp_inner) => {
                ExpKind::Un(op_inner, typ_op_inner, exp_inner.prepare(layout))
            }
            source::ExpKind::Bin(op, typ_op, exp_l, exp_r) => {
                ExpKind::Bin(op, typ_op, exp_l.prepare(layout), exp_r.prepare(layout))
            }
            source::ExpKind::Cmp(op, typ_op, exp_l, exp_r) => {
                ExpKind::Cmp(op, typ_op, exp_l.prepare(layout), exp_r.prepare(layout))
            }
            source::ExpKind::UpCast(typ_inner, exp_inner) => {
                ExpKind::UpCast(typ_inner, exp_inner.prepare(layout))
            }
            source::ExpKind::DownCast(typ_inner, exp_inner) => {
                ExpKind::DownCast(typ_inner, exp_inner.prepare(layout))
            }
            source::ExpKind::Sub(exp_inner, typ_inner, check_inner) => {
                ExpKind::Sub(exp_inner.prepare(layout), typ_inner, check_inner.prepare(layout))
            }
            source::ExpKind::Match(exp_inner, pattern_inner) => {
                ExpKind::Match(exp_inner.prepare(layout), pattern_inner.prepare(layout))
            }
            source::ExpKind::Tuple(exps_inner) => ExpKind::Tuple(exps_inner.prepare(layout)),
            source::ExpKind::Case(not_exp_inner) => ExpKind::Case(not_exp_inner.prepare(layout)),
            source::ExpKind::Str(exp_fields_inner) => {
                ExpKind::Str(exp_fields_inner.prepare(layout))
            }
            source::ExpKind::Opt(exp_opt_inner) => ExpKind::Opt(exp_opt_inner.prepare(layout)),
            source::ExpKind::List(exps_inner) => ExpKind::List(exps_inner.prepare(layout)),
            source::ExpKind::Cons(exp_head, exp_tail) => {
                ExpKind::Cons(exp_head.prepare(layout), exp_tail.prepare(layout))
            }
            source::ExpKind::Cat(exp_l, exp_r) => {
                ExpKind::Cat(exp_l.prepare(layout), exp_r.prepare(layout))
            }
            source::ExpKind::Mem(exp_elem, exp_list) => {
                ExpKind::Mem(exp_elem.prepare(layout), exp_list.prepare(layout))
            }
            source::ExpKind::Len(exp_inner) => ExpKind::Len(exp_inner.prepare(layout)),
            source::ExpKind::Dot(exp_inner, atom_inner) => {
                ExpKind::Dot(exp_inner.prepare(layout), atom_inner)
            }
            source::ExpKind::Idx(exp_base, exp_idx) => {
                ExpKind::Idx(exp_base.prepare(layout), exp_idx.prepare(layout))
            }
            source::ExpKind::Slice(exp_base, exp_idx, exp_len) => ExpKind::Slice(
                exp_base.prepare(layout),
                exp_idx.prepare(layout),
                exp_len.prepare(layout),
            ),
            source::ExpKind::Upd(exp_base, path_inner, exp_new) => ExpKind::Upd(
                exp_base.prepare(layout),
                path_inner.prepare(layout),
                exp_new.prepare(layout),
            ),
            source::ExpKind::Call(id_inner, targs_inner, args_inner) => {
                ExpKind::Call(id_inner, targs_inner, args_inner.prepare(layout))
            }
            source::ExpKind::Iter(exp_inner, exp_iter_inner) => {
                let exp_inner = exp_inner.prepare(layout);
                let exp_iter_inner = exp_iter_inner.prepare(layout);
                ExpKind::Iter(exp_inner, exp_iter_inner)
            }
        }
    }
}

impl Prepare for source::ExpField {
    type Output = ExpField;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        ExpField { atom: self.atom, exp: self.exp.prepare(layout) }
    }
}

impl Prepare for source::ExpIter {
    type Output = ExpIter;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        let source::ExpIter { iter, vars } = self;
        // Register `x*` for every iterated `x` so its slot exists
        let vars = vars.prepare(layout);
        for var in &vars {
            let mut var_outer = var.var.clone();
            var_outer.iters.push(iter);
            layout.resolve_var(var_outer);
        }
        ExpIter { iter, vars }
    }
}

// - Paths

impl Prepare for source::PathKind {
    type Output = PathKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::PathKind::Root => PathKind::Root,
            source::PathKind::Idx(path_base, exp_idx) => {
                PathKind::Idx(path_base.prepare(layout), exp_idx.prepare(layout))
            }
            source::PathKind::Slice(path_base, exp_idx, exp_len) => PathKind::Slice(
                path_base.prepare(layout),
                exp_idx.prepare(layout),
                exp_len.prepare(layout),
            ),
            source::PathKind::Dot(path_inner, atom_inner) => {
                PathKind::Dot(path_inner.prepare(layout), atom_inner)
            }
        }
    }
}

// - Arguments

impl Prepare for source::ArgKind {
    type Output = ArgKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::ArgKind::Exp(exp_inner) => ArgKind::Exp(exp_inner.prepare(layout)),
            source::ArgKind::Def(id_inner) => ArgKind::Def(id_inner),
        }
    }
}

// - Premises

impl Prepare for source::PremIter {
    type Output = PremIter;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        fn prepare_outer_vars(vars: &[VarSlot], iter: Iter, layout: &mut FrameLayout) {
            for var in vars {
                let mut var_outer = var.var.clone();
                var_outer.iters.push(iter);
                layout.resolve_var(var_outer);
            }
        }

        // Register the outer variables of the bound and binding variables
        let vars_bound = self.vars_bound.prepare(layout);
        let vars_bind = self.vars_bind.prepare(layout);
        prepare_outer_vars(&vars_bound, self.iter, layout);
        prepare_outer_vars(&vars_bind, self.iter, layout);
        PremIter { iter: self.iter, vars_bound, vars_bind }
    }
}
