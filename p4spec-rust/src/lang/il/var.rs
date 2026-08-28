//! Variable-to-expression conversion

use super::ast::*;

/// Converts `var` to an expression with the same type and iteration structure
///
/// Starts with a variable reference and wraps it once for each iterator in
/// `var.iters`, from innermost to outermost. When `is_dim` is true, each
/// iteration carries a binder describing that dimension; otherwise its binder
/// list is empty
pub fn as_exp(is_dim: bool, var: &Var) -> Exp {
    let mut exp = crate::noted! {
        kind: ExpKind::Var(var.id.clone()),
        note: var.typ.node.clone(),
        span: var.id,
    };
    let mut iters_prior = Vec::new();
    for iter in &var.iters {
        let typ_iter = crate::spanned! {
            node: TypKind::Iter(
                Box::new(crate::spanned! {
                    node: exp.node.note.clone(),
                    span: exp.span.clone(),
                }),
                *iter,
            ),
            span: var.typ.span.clone(),
        };
        let var_binder = Var {
            id: var.id.clone(),
            typ: typ_iter.clone(),
            iters: iters_prior.clone(),
        };
        exp = crate::noted! {
            kind: ExpKind::Iter(
                Box::new(exp),
                (*iter, if is_dim { vec![var_binder] } else { vec![] }),
            ),
            note: typ_iter.node.clone(),
            span: exp,
        };
        iters_prior.push(*iter);
    }
    exp
}
