//! Inference and annotation of variable iteration dimensions

use crate::{
    lang::{
        common::{
            Id,
            ds::map::IdMap,
            notation::mixfix::Mixfix,
            source::{Span, Spanned},
        },
        il::ast,
    },
    runtime::sta::{Dim, VEnv},
};

use super::{ElabError, ElabErrorKind};

#[derive(Clone, Debug, Default)]
struct DimContext(IdMap<Vec<Dim>>);

impl DimContext {
    fn add(&mut self, id: &Id, dim: Dim) {
        if let Some(dims) = self.0.get_mut(id) {
            dims.push(dim);
        } else {
            self.0.insert(id.clone(), vec![dim]);
        }
    }

    fn infer(self) -> Result<VEnv, ElabError> {
        let mut bounds = VEnv::new();
        for (id, dims) in self.0.iter() {
            let dim_min = dims
                .iter()
                .min_by_key(|dim| dim.iters().len())
                .expect("identifier has an occurrence");
            if dims.iter().any(|dim| !dim_min.sub(dim)) {
                return Err(ElabError::new(
                    ElabErrorKind::DimensionMismatch,
                    id.span.clone(),
                    format!(
                        "mismatched iteration dimensions for identifier `{}`",
                        id.node
                    ),
                ));
            }
            bounds.insert(id.clone(), dim_min.clone());
        }
        Ok(bounds)
    }
}

fn infer_exp(dims: &mut DimContext, exp: &ast::Exp, iters: &[ast::Iter]) -> Result<(), ElabError> {
    let typ = Spanned::new(exp.node.note.clone(), exp.span.clone());
    match &exp.node.kind {
        ast::ExpKind::Bool(_) | ast::ExpKind::Num(_) | ast::ExpKind::Text(_) => {}
        ast::ExpKind::Var(id) => dims.add(id, Dim::new(typ, iters.to_vec())),
        ast::ExpKind::Un(_, _, exp)
        | ast::ExpKind::UpCast(_, exp)
        | ast::ExpKind::DownCast(_, exp)
        | ast::ExpKind::Sub(exp, _, _)
        | ast::ExpKind::Match(exp, _)
        | ast::ExpKind::Len(exp)
        | ast::ExpKind::Dot(exp, _) => infer_exp(dims, exp, iters)?,
        ast::ExpKind::Bin(_, _, exp_l, exp_r)
        | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
        | ast::ExpKind::Cons(exp_l, exp_r)
        | ast::ExpKind::Cat(exp_l, exp_r)
        | ast::ExpKind::Mem(exp_l, exp_r)
        | ast::ExpKind::Idx(exp_l, exp_r) => {
            infer_exp(dims, exp_l, iters)?;
            infer_exp(dims, exp_r, iters)?;
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => {
            infer_exps(dims, exps, iters)?;
        }
        ast::ExpKind::Case(not_exp) => infer_not_exp(dims, not_exp, iters)?,
        ast::ExpKind::Str(fields) => {
            for (_, exp) in fields {
                infer_exp(dims, exp, iters)?;
            }
        }
        ast::ExpKind::Opt(exp) => {
            if let Some(exp) = exp {
                infer_exp(dims, exp, iters)?;
            }
        }
        ast::ExpKind::Slice(exp_base, exp_low, exp_high) => {
            infer_exp(dims, exp_base, iters)?;
            infer_exp(dims, exp_low, iters)?;
            infer_exp(dims, exp_high, iters)?;
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            infer_exp(dims, exp_base, iters)?;
            infer_path(dims, path, iters)?;
            infer_exp(dims, exp_field, iters)?;
        }
        ast::ExpKind::Call(_, _, args) => infer_args(dims, args, iters)?,
        ast::ExpKind::Iter(exp, (iter, _)) => {
            let mut iters_inner = Vec::with_capacity(iters.len() + 1);
            iters_inner.push(*iter);
            iters_inner.extend_from_slice(iters);
            infer_exp(dims, exp, &iters_inner)?;
        }
    }
    Ok(())
}

fn infer_exps(
    dims: &mut DimContext,
    exps: &[ast::Exp],
    iters: &[ast::Iter],
) -> Result<(), ElabError> {
    for exp in exps {
        infer_exp(dims, exp, iters)?;
    }
    Ok(())
}

fn infer_not_exp(
    dims: &mut DimContext,
    not_exp: &ast::NotExp,
    iters: &[ast::Iter],
) -> Result<(), ElabError> {
    let mut result = Ok(());
    not_exp.iter(|exp| {
        if result.is_ok() {
            result = infer_exp(dims, exp, iters);
        }
    });
    result
}

fn infer_path(
    dims: &mut DimContext,
    path: &ast::Path,
    iters: &[ast::Iter],
) -> Result<(), ElabError> {
    match &path.node.kind {
        ast::PathKind::Root => {}
        ast::PathKind::Idx(path, exp) => {
            infer_path(dims, path, iters)?;
            infer_exp(dims, exp, iters)?;
        }
        ast::PathKind::Slice(path, exp_l, exp_h) => {
            infer_path(dims, path, iters)?;
            infer_exp(dims, exp_l, iters)?;
            infer_exp(dims, exp_h, iters)?;
        }
        ast::PathKind::Dot(path, _) => infer_path(dims, path, iters)?,
    }
    Ok(())
}

fn infer_arg(dims: &mut DimContext, arg: &ast::Arg, iters: &[ast::Iter]) -> Result<(), ElabError> {
    if let ast::ArgKind::Exp(exp) = &arg.node {
        infer_exp(dims, exp, iters)?;
    }
    Ok(())
}

fn infer_args(
    dims: &mut DimContext,
    args: &[ast::Arg],
    iters: &[ast::Iter],
) -> Result<(), ElabError> {
    for arg in args {
        infer_arg(dims, arg, iters)?;
    }
    Ok(())
}

fn infer_prem(
    dims: &mut DimContext,
    prem: &ast::Prem,
    iters: &[ast::Iter],
) -> Result<(), ElabError> {
    match &prem.node {
        ast::PremKind::Rule(prem) => infer_not_exp(dims, &prem.not_exp, iters)?,
        ast::PremKind::If(prem) => infer_exp(dims, &prem.exp, iters)?,
        ast::PremKind::IfHold(prem) => infer_not_exp(dims, &prem.not_exp, iters)?,
        ast::PremKind::IfNotHold(prem) => infer_not_exp(dims, &prem.not_exp, iters)?,
        ast::PremKind::Let(_) => {
            return Err(ElabError::new(
                ElabErrorKind::MisplacedConstruct,
                prem.span.clone(),
                "let premise should appear only after algorithmic conversion",
            ));
        }
        ast::PremKind::Iter(prem_iter) => {
            if !prem_iter.iter_prem.vars_bound.is_empty()
                || !prem_iter.iter_prem.vars_bind.is_empty()
            {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIteration,
                    prem.span.clone(),
                    "iterated premise should initially have no annotations",
                ));
            }
            let mut iters_inner = Vec::with_capacity(iters.len() + 1);
            iters_inner.push(prem_iter.iter_prem.iter);
            iters_inner.extend_from_slice(iters);
            infer_prem(dims, &prem_iter.prem, &iters_inner)?;
        }
        ast::PremKind::Debug(prem) => infer_exp(dims, &prem.exp, iters)?,
    }
    Ok(())
}

fn infer_prems(dims: &mut DimContext, prems: &[ast::Prem]) -> Result<(), ElabError> {
    for prem in prems {
        infer_prem(dims, prem, &[])?;
    }
    Ok(())
}

fn infer_rule(rule: &ast::Rule) -> Result<DimContext, ElabError> {
    let mut dims = DimContext::default();
    infer_not_exp(&mut dims, &rule.node.not_exp, &[])?;
    infer_prems(&mut dims, &rule.node.prems)?;
    Ok(dims)
}

fn infer_clause(clause: &ast::Clause) -> Result<DimContext, ElabError> {
    let mut dims = DimContext::default();
    infer_args(&mut dims, &clause.node.args, &[])?;
    infer_prems(&mut dims, &clause.node.premises)?;
    infer_exp(&mut dims, &clause.node.expression, &[])?;
    Ok(dims)
}

fn infer_table_row(row: &ast::TableRow) -> Result<DimContext, ElabError> {
    let mut dims = DimContext::default();
    infer_args(&mut dims, &row.node.0, &[])?;
    infer_exp(&mut dims, &row.node.1, &[])?;
    Ok(dims)
}

#[cfg(test)]
mod tests {
    use crate::{
        lang::{
            common::{
                notation::mixfix::Mixfix,
                noted::Noted,
                source::{Position, Span, Spanned},
            },
            il::ast::{self, ExpKind, Iter, RuleKind, TypKind},
        },
        pass::elaborate::ElabErrorKind,
    };

    use super::infer_rule;

    fn span(label: &str) -> Span {
        Span::new(Position::new(label, 1, 0), Position::new(label, 1, 1))
    }

    fn id(name: &str, label: &str) -> ast::Id {
        Spanned::new(name.to_owned(), span(label))
    }

    fn exp(kind: ExpKind, typ: TypKind, label: &str) -> ast::Exp {
        Spanned::new(Noted::new(kind, typ), span(label))
    }

    fn variable(name: &str, label: &str) -> ast::Exp {
        exp(ExpKind::Var(id(name, label)), TypKind::Bool, label)
    }

    fn iter(exp_inner: ast::Exp, iter: Iter, label: &str) -> ast::Exp {
        exp(
            ExpKind::Iter(Box::new(exp_inner), (iter, vec![])),
            TypKind::Iter(Box::new(Spanned::new(TypKind::Bool, span(label))), iter),
            label,
        )
    }

    fn rule(not_exp: ast::NotExp) -> ast::Rule {
        Spanned::new(
            RuleKind {
                id: id("relation", "relation"),
                not_exp,
                prems: vec![],
            },
            span("rule"),
        )
    }

    #[test]
    fn inference_rejects_incompatible_iteration_bounds_at_the_identifier() {
        let variable_span = span("first-variable");
        let variable_l = exp(
            ExpKind::Var(Spanned::new("x".to_owned(), variable_span.clone())),
            TypKind::Bool,
            "left-variable",
        );
        let variable_r = variable("x", "second-variable");
        let rule = rule(Mixfix::Seq(vec![
            Mixfix::Arg(iter(variable_l, Iter::Opt, "optional")),
            Mixfix::Arg(iter(variable_r, Iter::List, "list")),
        ]));

        let error = infer_rule(&rule).and_then(|dims| dims.infer()).unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::DimensionMismatch);
        assert_eq!(error.span, variable_span);
    }

    #[test]
    fn inference_records_nested_dimensions_from_inner_to_outer() {
        let nested = iter(
            iter(variable("x", "variable"), Iter::Opt, "optional"),
            Iter::List,
            "list",
        );
        let rule = rule(Mixfix::Arg(nested));

        let bounds = infer_rule(&rule)
            .and_then(|dims| dims.infer())
            .expect("compatible dimensions");
        let dim = bounds.get(&id("x", "query")).expect("variable bound");

        assert_eq!(dim.iters(), &[Iter::Opt, Iter::List]);
        assert_eq!(dim.typ().node, TypKind::Bool);
    }
}
