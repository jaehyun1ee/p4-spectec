//! Slot instantiation of shared IL expression syntax

use super::{Prepare, restore_notation, restore_phrase};
use crate::lang::data::var::{IdSlot, VarSlot};
use crate::lang::il::ast as source;
pub use crate::lang::il::ast::{
    Atom, BinOp, CmpOp, DefTyp, DefTypKind, DefinedTyp, ExternTyp, FuncTyp, Hint, Id, Iter,
    ListPattern, Mixop, NotTyp, NotTypKind, Num, NumOp, OpTyp, OptPattern, Param, ParamKind,
    Pattern, Subcheck, TParam, Targ, TargKind, Text, Typ, TypCase, TypDef, TypField, TypKind,
    TypOrigin, TypOriginKind, UnOp, Value, ValueCase, ValueField, ValueKind, VarDef,
};
use crate::runtime::envs::interp::shared::frame::FrameLayout;

// == Prepared syntax

// - Variables

pub type Var = VarSlot;

// - Expressions

pub type Exp = source::Exp<IdSlot, VarSlot>;
pub type ExpKind = source::ExpKind<IdSlot, VarSlot>;
pub type NotExp = source::NotExp<IdSlot, VarSlot>;
pub type ExpField = source::ExpField<IdSlot, VarSlot>;
pub type ExpIter = source::ExpIter<VarSlot>;
pub type PremIter = source::PremIter<VarSlot>;

// - Paths

pub type Path = source::Path<IdSlot, VarSlot>;
pub type PathKind = source::PathKind<IdSlot, VarSlot>;

// - Arguments

pub type Arg = source::Arg<IdSlot, VarSlot>;
pub type ArgKind = source::ArgKind<IdSlot, VarSlot>;

// == Preparation

// == Expressions

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
                ExpKind::Sub(exp_inner.prepare(layout), typ_inner, check_inner)
            }
            source::ExpKind::Match(exp_inner, pattern_inner) => {
                ExpKind::Match(exp_inner.prepare(layout), pattern_inner)
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

// == Paths

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

// == Arguments

impl Prepare for source::ArgKind {
    type Output = ArgKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::ArgKind::Exp(exp_inner) => ArgKind::Exp(exp_inner.prepare(layout)),
            source::ArgKind::Def(id_inner) => ArgKind::Def(id_inner),
        }
    }
}

impl Prepare for source::ExpIter {
    type Output = ExpIter;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        let source::ExpIter { iter, vars } = self;
        let vars = vars.prepare(layout);
        prepare_outer_vars(&vars, iter, layout);
        ExpIter { iter, vars }
    }
}

impl Prepare for source::PremIter {
    type Output = PremIter;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        let vars_bound = self.vars_bound.prepare(layout);
        let vars_bind = self.vars_bind.prepare(layout);
        prepare_outer_vars(&vars_bound, self.iter, layout);
        prepare_outer_vars(&vars_bind, self.iter, layout);
        PremIter { iter: self.iter, vars_bound, vars_bind }
    }
}

fn prepare_outer_vars(vars: &[VarSlot], iter: Iter, layout: &mut FrameLayout) {
    for var in vars {
        let mut var_outer = var.var.clone();
        var_outer.iters.push(iter);
        layout.resolve_var(var_outer);
    }
}

// == Source reconstruction

pub fn restore_exp(exp: Exp) -> source::Exp {
    restore_phrase(exp, restore_exp_kind)
}

pub fn restore_not_exp(not_exp: NotExp) -> source::NotExp {
    restore_notation(not_exp, restore_exp)
}

pub fn restore_exp_field(exp_field: ExpField) -> source::ExpField {
    source::ExpField { atom: exp_field.atom, exp: restore_exp(exp_field.exp) }
}

pub fn restore_exp_iter(exp_iter: ExpIter) -> source::ExpIter {
    source::ExpIter {
        iter: exp_iter.iter,
        vars: exp_iter.vars.into_iter().map(|var| var.var).collect(),
    }
}

pub fn restore_path(path: Path) -> source::Path {
    restore_phrase(path, restore_path_kind)
}

pub fn restore_arg(arg: Arg) -> source::Arg {
    restore_phrase(arg, restore_arg_kind)
}

pub fn restore_exp_kind(exp_kind: ExpKind) -> source::ExpKind {
    match exp_kind {
        ExpKind::Bool(value_inner) => source::ExpKind::Bool(value_inner),
        ExpKind::Num(num_inner) => source::ExpKind::Num(num_inner),
        ExpKind::Text(text_inner) => source::ExpKind::Text(text_inner),
        ExpKind::Id(id_inner) => source::ExpKind::Id(id_inner.id),
        ExpKind::Un(op_inner, typ_op_inner, exp_inner) => {
            source::ExpKind::Un(op_inner, typ_op_inner, Box::new(restore_exp(*exp_inner)))
        }
        ExpKind::Bin(op, typ_op, exp_l, exp_r) => source::ExpKind::Bin(
            op,
            typ_op,
            Box::new(restore_exp(*exp_l)),
            Box::new(restore_exp(*exp_r)),
        ),
        ExpKind::Cmp(op, typ_op, exp_l, exp_r) => source::ExpKind::Cmp(
            op,
            typ_op,
            Box::new(restore_exp(*exp_l)),
            Box::new(restore_exp(*exp_r)),
        ),
        ExpKind::UpCast(typ_inner, exp_inner) => {
            source::ExpKind::UpCast(typ_inner, Box::new(restore_exp(*exp_inner)))
        }
        ExpKind::DownCast(typ_inner, exp_inner) => {
            source::ExpKind::DownCast(typ_inner, Box::new(restore_exp(*exp_inner)))
        }
        ExpKind::Sub(exp_inner, typ_inner, check_inner) => {
            source::ExpKind::Sub(Box::new(restore_exp(*exp_inner)), typ_inner, check_inner)
        }
        ExpKind::Match(exp_inner, pattern_inner) => {
            source::ExpKind::Match(Box::new(restore_exp(*exp_inner)), pattern_inner)
        }
        ExpKind::Tuple(exps_inner) => {
            source::ExpKind::Tuple(exps_inner.into_iter().map(restore_exp).collect())
        }
        ExpKind::Case(not_exp_inner) => {
            source::ExpKind::Case(Box::new(restore_not_exp(*not_exp_inner)))
        }
        ExpKind::Str(exp_fields_inner) => source::ExpKind::Str(
            exp_fields_inner
                .into_iter()
                .map(restore_exp_field)
                .collect(),
        ),
        ExpKind::Opt(exp_opt_inner) => {
            source::ExpKind::Opt(exp_opt_inner.map(|exp| Box::new(restore_exp(*exp))))
        }
        ExpKind::List(exps_inner) => {
            source::ExpKind::List(exps_inner.into_iter().map(restore_exp).collect())
        }
        ExpKind::Cons(exp_head, exp_tail) => source::ExpKind::Cons(
            Box::new(restore_exp(*exp_head)),
            Box::new(restore_exp(*exp_tail)),
        ),
        ExpKind::Cat(exp_l, exp_r) => {
            source::ExpKind::Cat(Box::new(restore_exp(*exp_l)), Box::new(restore_exp(*exp_r)))
        }
        ExpKind::Mem(exp_elem, exp_list) => {
            source::ExpKind::Mem(Box::new(restore_exp(*exp_elem)), Box::new(restore_exp(*exp_list)))
        }
        ExpKind::Len(exp_inner) => source::ExpKind::Len(Box::new(restore_exp(*exp_inner))),
        ExpKind::Dot(exp_inner, atom_inner) => {
            source::ExpKind::Dot(Box::new(restore_exp(*exp_inner)), atom_inner)
        }
        ExpKind::Idx(exp_base, exp_idx) => {
            source::ExpKind::Idx(Box::new(restore_exp(*exp_base)), Box::new(restore_exp(*exp_idx)))
        }
        ExpKind::Slice(exp_base, exp_idx, exp_len) => source::ExpKind::Slice(
            Box::new(restore_exp(*exp_base)),
            Box::new(restore_exp(*exp_idx)),
            Box::new(restore_exp(*exp_len)),
        ),
        ExpKind::Upd(exp_base, path_inner, exp_new) => source::ExpKind::Upd(
            Box::new(restore_exp(*exp_base)),
            Box::new(restore_path(*path_inner)),
            Box::new(restore_exp(*exp_new)),
        ),
        ExpKind::Call(id_inner, targs_inner, args_inner) => source::ExpKind::Call(
            id_inner,
            targs_inner,
            args_inner.into_iter().map(restore_arg).collect(),
        ),
        ExpKind::Iter(exp_inner, exp_iter_inner) => source::ExpKind::Iter(
            Box::new(restore_exp(*exp_inner)),
            restore_exp_iter(exp_iter_inner),
        ),
    }
}

pub fn restore_path_kind(path_kind: PathKind) -> source::PathKind {
    match path_kind {
        PathKind::Root => source::PathKind::Root,
        PathKind::Idx(path_base, exp_idx) => source::PathKind::Idx(
            Box::new(restore_path(*path_base)),
            Box::new(restore_exp(*exp_idx)),
        ),
        PathKind::Slice(path_base, exp_idx, exp_len) => source::PathKind::Slice(
            Box::new(restore_path(*path_base)),
            Box::new(restore_exp(*exp_idx)),
            Box::new(restore_exp(*exp_len)),
        ),
        PathKind::Dot(path_inner, atom_inner) => {
            source::PathKind::Dot(Box::new(restore_path(*path_inner)), atom_inner)
        }
    }
}

pub fn restore_arg_kind(arg_kind: ArgKind) -> source::ArgKind {
    match arg_kind {
        ArgKind::Exp(exp_inner) => source::ArgKind::Exp(Box::new(restore_exp(*exp_inner))),
        ArgKind::Def(id_inner) => source::ArgKind::Def(id_inner),
    }
}

pub fn restore_prem_iter(prem_iter: PremIter) -> source::PremIter {
    source::PremIter {
        iter: prem_iter.iter,
        vars_bound: prem_iter
            .vars_bound
            .into_iter()
            .map(|var| var.var)
            .collect(),
        vars_bind: prem_iter.vars_bind.into_iter().map(|var| var.var).collect(),
    }
}
