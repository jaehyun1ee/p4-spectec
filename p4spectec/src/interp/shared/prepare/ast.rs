//! Slot instantiation of shared IL syntax
//!
//! `Prepared` is the IL stage with frame slots for identifiers and variables;
//! the type aliases name its forms, and the `Prepare` impls rewrite each node.
//! Iterations also register the outer variable `x*` for every iterated `x`,
//! so `eval::iter` can find its slot.

use crate::lang::data::{
    notation::{Mixfix, MixopArena, MixopId},
    var::{IdSlot, VarSlot},
};

use crate::lang::il::ast::{self as source, Stage};

pub use crate::lang::il::ast::{
    Atom, BinOp, CmpOp, DefinedTyp, ExternTyp, FuncTyp, Hint, Id, Iter, ListPattern, MixopTree,
    Num, NumOp, OpTyp, OptPattern, Param, ParamKind, TParam, Targ, TargKind, Text, Typ, TypDef,
    TypField, TypKind, TypOrigin, TypOriginKind, UnOp, Value, ValueCase, ValueField, ValueFlatKind,
    VarDef,
};

use crate::runtime::envs::interp::shared::frame::FrameLayout;

use super::{Prepare, PrepareContext, prepare_mixop};

// == Prepared syntax

// - Stage

/// IL syntax whose identifiers and variables are resolved to frame slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prepared;

impl Stage for Prepared {
    type Id = IdSlot;
    type Var = VarSlot;
    type Mixop = MixopId;
}

// - Variables

/// A variable with its frame slot.
pub type Var = VarSlot;

// - Expressions

/// An expression over slot-resolved identifiers.
///
/// Rendering requires its notation arena; ordinary `Print` is source-only.
///
/// ```compile_fail
/// use p4spectec::{interp::shared::prepare::ast::Exp, lang::traits::print::Print};
/// fn without_arena(exp: &Exp) -> String { Print::to_string(exp) }
/// ```
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

// - Type definitions

/// A notation type over a prepared shape.
pub type NotTyp = source::NotTyp<Prepared>;
pub type NotTypKind = source::NotTypKind<Prepared>;
/// A type definition body whose case notations are shapes.
pub type DefTyp = source::DefTyp<Prepared>;
pub type DefTypKind = source::DefTypKind<Prepared>;
pub type TypCase = source::TypCase<Prepared>;
/// What a type name resolves to while interpreting.
pub type TypeDef = crate::runtime::typdef::TypeDef<Prepared>;

// - Patterns and subtype checks

/// A pattern over prepared mixops.
pub type Pattern = source::Pattern<Prepared>;
/// A runtime subtype check over prepared mixops.
pub type Subcheck = source::Subcheck<Prepared>;

// - Premises

/// A premise iterator over slot-resolved variables.
pub type PremIter = source::PremIter<VarSlot>;

// == Preparation

// - Type definitions

/// Prepares a type definition body, interning its case notations as shapes.
///
/// Type definitions have no frame of their own, so only `arena_mixop` is filled.
pub fn prepare_def_typ(def_typ: source::DefTyp, arena_mixop: &mut MixopArena) -> DefTyp {
    let def_typ_kind = match def_typ.node {
        source::DefTypKind::Plain(typ) => DefTypKind::Plain(typ),
        source::DefTypKind::Struct(typ_fields) => DefTypKind::Struct(typ_fields),
        source::DefTypKind::Variant(typ_cases) => DefTypKind::Variant(
            typ_cases
                .into_iter()
                .map(|typ_case| prepare_typ_case(typ_case, arena_mixop))
                .collect(),
        ),
    };
    crate::phrase!(node: def_typ_kind, span: def_typ.span)
}

/// Prepares one variant case, interning its notation as a shape.
fn prepare_typ_case(typ_case: source::TypCase, arena_mixop: &mut MixopArena) -> TypCase {
    let source::TypCase { not_typ, typ_origin, hints } = typ_case;
    let (mixop, typs) = not_typ.node.into_parts();
    let shape = arena_mixop
        .intern_shared(&mixop)
        .expect("specification mixops fit in 32-bit shape handles");
    let not_typ_kind =
        Mixfix::new_in(arena_mixop, shape, typs).expect("a notation type fills every position");
    TypCase { not_typ: crate::phrase!(node: not_typ_kind, span: not_typ.span), typ_origin, hints }
}

// - Patterns and subtype checks

impl Prepare for source::Pattern {
    type Output = Pattern;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::Pattern::Case(mixop) => Pattern::Case(prepare_mixop(&mixop, ctx)),
            source::Pattern::List(pattern) => Pattern::List(pattern),
            source::Pattern::Opt(pattern) => Pattern::Opt(pattern),
        }
    }
}

impl Prepare for source::Subcheck {
    type Output = Subcheck;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::Subcheck::Skip => Subcheck::Skip,
            source::Subcheck::Mixop(mixops) => Subcheck::Mixop(
                mixops
                    .iter()
                    .map(|mixop| prepare_mixop(mixop, ctx))
                    .collect(),
            ),
            source::Subcheck::Tuple(subchecks) => Subcheck::Tuple(subchecks.prepare(ctx)),
            source::Subcheck::Iter(iter, subcheck) => Subcheck::Iter(iter, subcheck.prepare(ctx)),
            source::Subcheck::Recurse(typ) => Subcheck::Recurse(typ),
        }
    }
}
// - Expressions

impl Prepare for source::ExpKind {
    type Output = ExpKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::ExpKind::Bool(value_inner) => ExpKind::Bool(value_inner),
            source::ExpKind::Num(num_inner) => ExpKind::Num(num_inner),
            source::ExpKind::Text(text_inner) => ExpKind::Text(text_inner),
            source::ExpKind::Id(id_inner) => ExpKind::Id(id_inner.prepare(ctx)),
            source::ExpKind::Un(op_inner, typ_op_inner, exp_inner) => {
                ExpKind::Un(op_inner, typ_op_inner, exp_inner.prepare(ctx))
            }
            source::ExpKind::Bin(op, typ_op, exp_l, exp_r) => {
                ExpKind::Bin(op, typ_op, exp_l.prepare(ctx), exp_r.prepare(ctx))
            }
            source::ExpKind::Cmp(op, typ_op, exp_l, exp_r) => {
                ExpKind::Cmp(op, typ_op, exp_l.prepare(ctx), exp_r.prepare(ctx))
            }
            source::ExpKind::UpCast(typ_inner, exp_inner) => {
                ExpKind::UpCast(typ_inner, exp_inner.prepare(ctx))
            }
            source::ExpKind::DownCast(typ_inner, exp_inner) => {
                ExpKind::DownCast(typ_inner, exp_inner.prepare(ctx))
            }
            source::ExpKind::Sub(exp_inner, typ_inner, check_inner) => {
                ExpKind::Sub(exp_inner.prepare(ctx), typ_inner, check_inner.prepare(ctx))
            }
            source::ExpKind::Match(exp_inner, pattern_inner) => {
                ExpKind::Match(exp_inner.prepare(ctx), pattern_inner.prepare(ctx))
            }
            source::ExpKind::Tuple(exps_inner) => ExpKind::Tuple(exps_inner.prepare(ctx)),
            source::ExpKind::Case(not_exp_inner) => ExpKind::Case(not_exp_inner.prepare(ctx)),
            source::ExpKind::Str(exp_fields_inner) => ExpKind::Str(exp_fields_inner.prepare(ctx)),
            source::ExpKind::Opt(exp_opt_inner) => ExpKind::Opt(exp_opt_inner.prepare(ctx)),
            source::ExpKind::List(exps_inner) => ExpKind::List(exps_inner.prepare(ctx)),
            source::ExpKind::Cons(exp_head, exp_tail) => {
                ExpKind::Cons(exp_head.prepare(ctx), exp_tail.prepare(ctx))
            }
            source::ExpKind::Cat(exp_l, exp_r) => {
                ExpKind::Cat(exp_l.prepare(ctx), exp_r.prepare(ctx))
            }
            source::ExpKind::Mem(exp_elem, exp_list) => {
                ExpKind::Mem(exp_elem.prepare(ctx), exp_list.prepare(ctx))
            }
            source::ExpKind::Len(exp_inner) => ExpKind::Len(exp_inner.prepare(ctx)),
            source::ExpKind::Dot(exp_inner, atom_inner) => {
                ExpKind::Dot(exp_inner.prepare(ctx), atom_inner)
            }
            source::ExpKind::Idx(exp_base, exp_idx) => {
                ExpKind::Idx(exp_base.prepare(ctx), exp_idx.prepare(ctx))
            }
            source::ExpKind::Slice(exp_base, exp_idx, exp_len) => {
                ExpKind::Slice(exp_base.prepare(ctx), exp_idx.prepare(ctx), exp_len.prepare(ctx))
            }
            source::ExpKind::Upd(exp_base, path_inner, exp_new) => {
                ExpKind::Upd(exp_base.prepare(ctx), path_inner.prepare(ctx), exp_new.prepare(ctx))
            }
            source::ExpKind::Call(id_inner, targs_inner, args_inner) => {
                ExpKind::Call(id_inner, targs_inner, args_inner.prepare(ctx))
            }
            source::ExpKind::Iter(exp_inner, exp_iter_inner) => {
                let exp_inner = exp_inner.prepare(ctx);
                let exp_iter_inner = exp_iter_inner.prepare(ctx);
                ExpKind::Iter(exp_inner, exp_iter_inner)
            }
        }
    }
}

impl Prepare for source::ExpField {
    type Output = ExpField;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ExpField { atom: self.atom, exp: self.exp.prepare(ctx) }
    }
}

impl Prepare for source::ExpIter {
    type Output = ExpIter;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        let source::ExpIter { iter, vars } = self;
        // Register `x*` for every iterated `x` so its slot exists
        let vars = vars.prepare(ctx);
        for var in &vars {
            let mut var_outer = var.var.clone();
            var_outer.iters.push(iter);
            ctx.layout.resolve_var(var_outer);
        }
        ExpIter { iter, vars }
    }
}

// - Paths

impl Prepare for source::PathKind {
    type Output = PathKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::PathKind::Root => PathKind::Root,
            source::PathKind::Idx(path_base, exp_idx) => {
                PathKind::Idx(path_base.prepare(ctx), exp_idx.prepare(ctx))
            }
            source::PathKind::Slice(path_base, exp_idx, exp_len) => {
                PathKind::Slice(path_base.prepare(ctx), exp_idx.prepare(ctx), exp_len.prepare(ctx))
            }
            source::PathKind::Dot(path_inner, atom_inner) => {
                PathKind::Dot(path_inner.prepare(ctx), atom_inner)
            }
        }
    }
}

// - Arguments

impl Prepare for source::ArgKind {
    type Output = ArgKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::ArgKind::Exp(exp_inner) => ArgKind::Exp(exp_inner.prepare(ctx)),
            source::ArgKind::Def(id_inner) => ArgKind::Def(id_inner),
        }
    }
}

// - Premises

impl Prepare for source::PremIter {
    type Output = PremIter;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        fn prepare_outer_vars(vars: &[VarSlot], iter: Iter, layout: &mut FrameLayout) {
            for var in vars {
                let mut var_outer = var.var.clone();
                var_outer.iters.push(iter);
                layout.resolve_var(var_outer);
            }
        }

        // Register the outer variables of the bound and binding variables
        let vars_bound = self.vars_bound.prepare(ctx);
        let vars_bind = self.vars_bind.prepare(ctx);
        prepare_outer_vars(&vars_bound, self.iter, ctx.layout);
        prepare_outer_vars(&vars_bind, self.iter, ctx.layout);
        PremIter { iter: self.iter, vars_bound, vars_bind }
    }
}
