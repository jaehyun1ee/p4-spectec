//! Resolve simple iterated-variable expressions through their callable layout

use super::{context::ReadContext, prepare::expr as ast};
use crate::lang::data::var::VarSlot;

/// Finds the slot-backed variable represented by a simple iterated expression
pub fn find_var(ctx: &impl ReadContext, exp: &ast::Exp) -> Option<VarSlot> {
    match &exp.node {
        ast::ExpKind::Id(id) => Some(VarSlot {
            slot: id.slot,
            var: crate::lang::data::var::Var {
                id: id.id.clone(),
                typ: crate::phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone()),
                iters: vec![],
            },
        }),
        ast::ExpKind::Iter(exp_inner, ast::ExpIter { iter, vars }) => {
            let [var] = vars.as_slice() else {
                return None;
            };
            let var_inner = find_var(ctx, exp_inner)?;
            if var_inner.var.id.node != var.var.id.node || var_inner.var.iters != var.var.iters {
                return None;
            }
            Some(ctx.find_iter_var(&var_inner, *iter))
        }
        _ => None,
    }
}

/// Advances prepared variables through one iterator dimension
pub fn iterate_vars(ctx: &impl ReadContext, vars: &[ast::Var], iter: ast::Iter) -> Vec<ast::Var> {
    vars.iter()
        .map(|var| ctx.find_iter_var(var, iter))
        .collect()
}
