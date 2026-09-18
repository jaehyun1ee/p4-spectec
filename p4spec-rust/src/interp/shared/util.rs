//! Resolve simple iterated-variable expressions through their callable layout

use super::{context::ReadContext, prepare::expr as ast};
use crate::lang::data::var::VarSlot;

pub fn find_iter_var_slot(ctx: &impl ReadContext, exp: &ast::Exp) -> Option<VarSlot> {
    match &exp.node {
        ast::ExpKind::Id(id) => Some(VarSlot {
            slot: id.slot,
            var: crate::lang::data::var::Var {
                id: id.id.clone(),
                typ: crate::phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone()),
                iters: vec![],
            },
        }),
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            let [var] = vars.as_slice() else {
                return None;
            };
            let slot_inner = find_iter_var_slot(ctx, exp_inner)?;
            if slot_inner.var.id.node != var.var.id.node || slot_inner.var.iters != var.var.iters {
                return None;
            }
            Some(ctx.iter_slot(&slot_inner, *iter))
        }
        _ => None,
    }
}

pub fn iter_vars(ctx: &impl ReadContext, vars: &[ast::Var], iter: ast::Iter) -> Vec<ast::Var> {
    vars.iter().map(|var| ctx.iter_slot(var, iter)).collect()
}
