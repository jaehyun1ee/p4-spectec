//! Variables for internal language data

use super::ast::*;
pub fn as_exp(var: &Var, dim: bool) -> Exp {
    let (id, typ, iters) = var;
    let mut exp = Exp::new(ExpKind::VarE(id.clone()), typ.node.clone(), id.span.clone());
    let mut prior = Vec::new();
    for iter in iters {
        let exp_span = exp.span.clone();
        let iter_typ = Typ::new(
            TypKind::IterT(Box::new(Typ::new(exp.ty.clone(), exp_span.clone())), *iter),
            typ.span.clone(),
        );
        let binder = (id.clone(), iter_typ.clone(), prior.clone());
        exp = Exp::new(
            ExpKind::IterE(
                Box::new(exp),
                (*iter, if dim { vec![binder] } else { vec![] }),
            ),
            iter_typ.node.clone(),
            exp_span,
        );
        prior.push(*iter);
    }
    exp
}
