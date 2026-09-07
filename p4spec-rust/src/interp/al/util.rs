//! Iterated-variable recognition shared by assignment and evaluation

use crate::lang::{al::ast, common::Variable};

pub fn is_iter_var_exp(exp: &ast::Exp) -> Option<Variable> {
    match &exp.node {
        ast::ExpKind::Var(id) => Some(Variable::new(id.clone(), vec![])),
        ast::ExpKind::Iter(exp, (iter, vars)) => {
            let mut var = is_iter_var_exp(exp)?;
            let [binding] = vars.as_slice() else {
                return None;
            };
            if var.id.node != binding.id.node || var.iters != binding.iters {
                return None;
            }
            var.iters.push(*iter);
            Some(var)
        }
        _ => None,
    }
}
