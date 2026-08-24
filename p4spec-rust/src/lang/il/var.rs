use super::ast::*;
pub fn as_exp(var: &Var, dim: bool) -> Exp {
    let mut exp = Exp::new(
        ExpKind::VarE(var.id.clone()),
        var.typ.node.clone(),
        var.id.span.clone(),
    );
    let mut prior = Vec::new();
    for iter in &var.iters {
        let exp_span = exp.span.clone();
        let iter_typ = Typ::new(
            TypKind::IterT(Box::new(Typ::new(exp.ty.clone(), exp_span.clone())), *iter),
            var.typ.span.clone(),
        );
        let binder = Var {
            id: var.id.clone(),
            typ: iter_typ.clone(),
            iters: prior.clone(),
        };
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
