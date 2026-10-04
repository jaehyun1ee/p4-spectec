//! Resolve simple iterated-variable expressions through their callable layout
//!
//! `find_var_of_exp` maps `x*` used as an expression back to the slot of the iterated
//! variable; `iterate_vars` computes the slots one iteration outward.

use crate::lang::data::{
    typ,
    value::TypeNote,
    var::{SlotIdx, Var, VarSlot},
};

use super::{context::ReadContext, prepare::ast};

/// A variable borrowed one iteration outward, with its resolved slot.
#[derive(Clone, Copy, Debug)]
pub struct VarIter<'a> {
    /// Prepared slot one iteration outward.
    pub slot: SlotIdx,
    /// Inner variable whose type and iteration path are borrowed.
    var: &'a VarSlot,
    /// The additional iteration around the inner type.
    iter: ast::Iter,
}

impl VarIter<'_> {
    /// Chooses an annotation recipe without reserving an arena identity yet.
    pub(crate) fn note(&self, ctx: &impl ReadContext) -> TypeNote {
        match ctx.find_iterated_type_template(self.var, self.iter) {
            Some(typ_template) => TypeNote::FreshClone(typ_template.clone()),
            None => TypeNote::Shared(self.typ().node.into()),
        }
    }

    /// Builds the outer type only when an output value needs it.
    pub fn typ(&self) -> ast::Typ {
        let typ = typ::make::iterate(self.var.var.typ.clone(), &self.var.var.iters);
        typ::make::iterate(typ, &[self.iter])
    }
}

/// Advances slots while borrowing the variables' types and iteration paths.
pub fn iterate_vars<'a>(
    ctx: &impl ReadContext,
    vars: &'a [ast::Var],
    iter: ast::Iter,
) -> Vec<VarIter<'a>> {
    vars.iter()
        .map(|var| VarIter { slot: ctx.find_slot_iterated(var, iter), var, iter })
        .collect()
}

/// Recognizes identity iterations using prepared slots without cloning syntax.
pub fn find_slot_of_exp(ctx: &impl ReadContext, exp: &ast::Exp) -> Option<SlotIdx> {
    match &exp.node {
        // A plain variable already has its slot
        ast::ExpKind::Id(id) => Some(id.slot),
        // Matching slots imply the same name and the same iteration path
        ast::ExpKind::Iter(exp_inner, ast::ExpIter { iter, vars }) => {
            let [var] = vars.as_slice() else {
                return None;
            };
            let slot = find_slot_of_exp(ctx, exp_inner)?;
            (slot == var.slot).then(|| ctx.find_slot_iterated(var, *iter))
        }
        // Computed expressions require normal iteration evaluation
        _ => None,
    }
}

/// Finds the slot-backed variable represented by a simple iterated expression.
pub fn find_var_of_exp(ctx: &impl ReadContext, exp: &ast::Exp) -> Option<VarSlot> {
    match &exp.node {
        // A plain variable is its own slot
        ast::ExpKind::Id(id) => Some(VarSlot {
            slot: id.slot,
            var: Var {
                id: id.id.clone(),
                typ: crate::phrase!(node: exp.note.as_ref().clone(), span: exp.span),
                iters: vec![],
            },
        }),
        // `x*` must be a single-variable iteration over `x` itself
        ast::ExpKind::Iter(exp_inner, ast::ExpIter { iter, vars }) => {
            let [var] = vars.as_slice() else {
                return None;
            };
            let var_inner = find_var_of_exp(ctx, exp_inner)?;
            if var_inner.var.id.node != var.var.id.node || var_inner.var.iters != var.var.iters {
                return None;
            }
            Some(ctx.find_var_iterated(&var_inner, *iter))
        }
        // Anything else is a computed expression
        _ => None,
    }
}
