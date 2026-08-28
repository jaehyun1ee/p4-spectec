//! Inference and annotation of variable iteration dimensions

use crate::{
    lang::{
        common::{Id, ds::map::IdMap, notation::mixfix::Mixfix, noted::Noted, source::Spanned},
        il::ast,
        traits::eq::SyntaxEq,
    },
    runtime::sta::{Dim, VEnv},
};

use super::{ElabError, ElabErrorKind};

#[derive(Clone, Debug, Default)]
struct DimContext(IdMap<Vec<Dim>>);

impl DimContext {
    fn add(&mut self, id: &Id, dim: Dim) {
        let mut dims = self.0.remove(id).unwrap_or_default();
        dims.push(dim);
        self.0.insert(id.clone(), dims);
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

fn singleton(id: &Id, typ: ast::Typ) -> VEnv {
    let mut occurs = VEnv::new();
    occurs.insert(id.clone(), Dim::new(typ, vec![]));
    occurs
}

fn union(mut occurs_l: VEnv, occurs_r: VEnv) -> Result<VEnv, ElabError> {
    for (id, dim_r) in occurs_r.iter() {
        if let Some(dim_l) = occurs_l.get(id) {
            if !dim_l.typ().syntax_eq(dim_r.typ()) {
                return Err(ElabError::new(
                    ElabErrorKind::TypeMismatch,
                    id.span.clone(),
                    format!("type mismatch for identifier `{}` in union", id.node),
                ));
            }
            if dim_r.iters().len() <= dim_l.iters().len() {
                occurs_l.insert(id.clone(), dim_r.clone());
            }
        } else {
            occurs_l.insert(id.clone(), dim_r.clone());
        }
    }
    Ok(occurs_l)
}

fn iterate_occurs(occurs: VEnv, vars: &[ast::Var], iter: ast::Iter) -> VEnv {
    let mut iterated = occurs;
    for var in vars {
        let mut iters = var.iters.clone();
        iters.push(iter);
        iterated.insert(var.id.clone(), Dim::new(var.typ.clone(), iters));
    }
    iterated
}

fn collect_iter_vars(bounds: &VEnv, occurs: &VEnv, iter: ast::Iter) -> Vec<ast::Var> {
    occurs
        .iter()
        .filter_map(|(id, dim)| {
            let expected = bounds
                .get(id)
                .expect("occurring variable has inferred bound");
            if dim.clone().add_iter(iter).sub(expected) {
                Some(ast::Var {
                    id: id.clone(),
                    typ: dim.typ().clone(),
                    iters: dim.iters().to_vec(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn annotate_exp(bounds: &VEnv, exp: &ast::Exp) -> Result<(VEnv, ast::Exp), ElabError> {
    let span = exp.span.clone();
    let note = exp.node.note.clone();
    let (occurs, kind) = match &exp.node.kind {
        ast::ExpKind::Bool(value) => (VEnv::new(), ast::ExpKind::Bool(*value)),
        ast::ExpKind::Num(value) => (VEnv::new(), ast::ExpKind::Num(value.clone())),
        ast::ExpKind::Text(value) => (VEnv::new(), ast::ExpKind::Text(value.clone())),
        ast::ExpKind::Var(id) => {
            let typ = Spanned::new(note.clone(), span.clone());
            (singleton(id, typ), ast::ExpKind::Var(id.clone()))
        }
        ast::ExpKind::Un(op, op_typ, exp_inner) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (occurs, ast::ExpKind::Un(*op, *op_typ, Box::new(exp_inner)))
        }
        ast::ExpKind::Bin(op, op_typ, exp_l, exp_r) => {
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_r, exp_r) = annotate_exp(bounds, exp_r)?;
            (
                union(occurs_l, occurs_r)?,
                ast::ExpKind::Bin(*op, *op_typ, Box::new(exp_l), Box::new(exp_r)),
            )
        }
        ast::ExpKind::Cmp(op, op_typ, exp_l, exp_r) => {
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_r, exp_r) = annotate_exp(bounds, exp_r)?;
            (
                union(occurs_l, occurs_r)?,
                ast::ExpKind::Cmp(*op, *op_typ, Box::new(exp_l), Box::new(exp_r)),
            )
        }
        ast::ExpKind::UpCast(typ, exp_inner) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (
                occurs,
                ast::ExpKind::UpCast(typ.clone(), Box::new(exp_inner)),
            )
        }
        ast::ExpKind::DownCast(typ, exp_inner) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (
                occurs,
                ast::ExpKind::DownCast(typ.clone(), Box::new(exp_inner)),
            )
        }
        ast::ExpKind::Sub(exp_inner, typ, check) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (
                occurs,
                ast::ExpKind::Sub(Box::new(exp_inner), typ.clone(), check.clone()),
            )
        }
        ast::ExpKind::Match(exp_inner, pattern) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (
                occurs,
                ast::ExpKind::Match(Box::new(exp_inner), pattern.clone()),
            )
        }
        ast::ExpKind::Tuple(exps) => {
            let (occurs, exps) = annotate_exps(bounds, exps)?;
            (occurs, ast::ExpKind::Tuple(exps))
        }
        ast::ExpKind::Case(not_exp) => {
            let (occurs, not_exp) = annotate_not_exp(bounds, not_exp)?;
            (occurs, ast::ExpKind::Case(Box::new(not_exp)))
        }
        ast::ExpKind::Str(fields) => {
            let atoms = fields
                .iter()
                .map(|(atom, _)| atom.clone())
                .collect::<Vec<_>>();
            let exps = fields
                .iter()
                .map(|(_, exp)| exp.clone())
                .collect::<Vec<_>>();
            let (occurs, exps) = annotate_exps(bounds, &exps)?;
            (
                occurs,
                ast::ExpKind::Str(atoms.into_iter().zip(exps).collect()),
            )
        }
        ast::ExpKind::Opt(Some(exp_inner)) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (occurs, ast::ExpKind::Opt(Some(Box::new(exp_inner))))
        }
        ast::ExpKind::Opt(None) => (VEnv::new(), ast::ExpKind::Opt(None)),
        ast::ExpKind::List(exps) => {
            let (occurs, exps) = annotate_exps(bounds, exps)?;
            (occurs, ast::ExpKind::List(exps))
        }
        ast::ExpKind::Cons(exp_l, exp_r) => {
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_r, exp_r) = annotate_exp(bounds, exp_r)?;
            (
                union(occurs_l, occurs_r)?,
                ast::ExpKind::Cons(Box::new(exp_l), Box::new(exp_r)),
            )
        }
        ast::ExpKind::Cat(exp_l, exp_r) => {
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_r, exp_r) = annotate_exp(bounds, exp_r)?;
            (
                union(occurs_l, occurs_r)?,
                ast::ExpKind::Cat(Box::new(exp_l), Box::new(exp_r)),
            )
        }
        ast::ExpKind::Mem(exp_l, exp_r) => {
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_r, exp_r) = annotate_exp(bounds, exp_r)?;
            (
                union(occurs_l, occurs_r)?,
                ast::ExpKind::Mem(Box::new(exp_l), Box::new(exp_r)),
            )
        }
        ast::ExpKind::Len(exp_inner) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (occurs, ast::ExpKind::Len(Box::new(exp_inner)))
        }
        ast::ExpKind::Dot(exp_inner, atom) => {
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            (occurs, ast::ExpKind::Dot(Box::new(exp_inner), atom.clone()))
        }
        ast::ExpKind::Idx(exp_l, exp_r) => {
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_r, exp_r) = annotate_exp(bounds, exp_r)?;
            (
                union(occurs_l, occurs_r)?,
                ast::ExpKind::Idx(Box::new(exp_l), Box::new(exp_r)),
            )
        }
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            let (occurs_base, exp_base) = annotate_exp(bounds, exp_base)?;
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_h, exp_h) = annotate_exp(bounds, exp_h)?;
            (
                union(union(occurs_base, occurs_l)?, occurs_h)?,
                ast::ExpKind::Slice(Box::new(exp_base), Box::new(exp_l), Box::new(exp_h)),
            )
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            let (occurs_base, exp_base) = annotate_exp(bounds, exp_base)?;
            let (occurs_field, exp_field) = annotate_exp(bounds, exp_field)?;
            let (occurs_path, path) = annotate_path(bounds, path)?;
            (
                union(union(occurs_base, occurs_field)?, occurs_path)?,
                ast::ExpKind::Upd(Box::new(exp_base), path, Box::new(exp_field)),
            )
        }
        ast::ExpKind::Call(id, targs, args) => {
            let (occurs, args) = annotate_args(bounds, args)?;
            (occurs, ast::ExpKind::Call(id.clone(), targs.clone(), args))
        }
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            if !vars.is_empty() {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIteration,
                    span,
                    "iterated expression should initially have no annotations",
                ));
            }
            let (occurs, exp_inner) = annotate_exp(bounds, exp_inner)?;
            let vars = collect_iter_vars(bounds, &occurs, *iter);
            if vars.is_empty() {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIteration,
                    span,
                    "empty iteration",
                ));
            }
            let occurs = iterate_occurs(occurs, &vars, *iter);
            (
                occurs,
                ast::ExpKind::Iter(Box::new(exp_inner), (*iter, vars)),
            )
        }
    };
    let exp = Spanned::new(Noted::new(kind, note), exp.span.clone());
    Ok((occurs, exp))
}

fn annotate_exps(bounds: &VEnv, exps: &[ast::Exp]) -> Result<(VEnv, Vec<ast::Exp>), ElabError> {
    let mut occurs = VEnv::new();
    let mut annotated = Vec::with_capacity(exps.len());
    for exp in exps {
        let (occurs_exp, exp) = annotate_exp(bounds, exp)?;
        occurs = union(occurs, occurs_exp)?;
        annotated.push(exp);
    }
    Ok((occurs, annotated))
}

fn annotate_not_exp(
    bounds: &VEnv,
    not_exp: &ast::NotExp,
) -> Result<(VEnv, ast::NotExp), ElabError> {
    match not_exp {
        Mixfix::Arg(exp) => {
            let (occurs, exp) = annotate_exp(bounds, exp)?;
            Ok((occurs, Mixfix::Arg(exp)))
        }
        Mixfix::Atom(atom) => Ok((VEnv::new(), Mixfix::Atom(atom.clone()))),
        Mixfix::Brack(atom_l, not_exp, atom_r) => {
            let (occurs, not_exp) = annotate_not_exp(bounds, not_exp)?;
            Ok((
                occurs,
                Mixfix::Brack(atom_l.clone(), Box::new(not_exp), atom_r.clone()),
            ))
        }
        Mixfix::Infix(not_exp_l, atom, not_exp_r) => {
            let (occurs_l, not_exp_l) = annotate_not_exp(bounds, not_exp_l)?;
            let (occurs_r, not_exp_r) = annotate_not_exp(bounds, not_exp_r)?;
            Ok((
                union(occurs_l, occurs_r)?,
                Mixfix::Infix(Box::new(not_exp_l), atom.clone(), Box::new(not_exp_r)),
            ))
        }
        Mixfix::Seq(not_exps) => {
            let mut occurs = VEnv::new();
            let mut annotated = Vec::with_capacity(not_exps.len());
            for not_exp in not_exps {
                let (occurs_exp, not_exp) = annotate_not_exp(bounds, not_exp)?;
                occurs = union(occurs, occurs_exp)?;
                annotated.push(not_exp);
            }
            Ok((occurs, Mixfix::Seq(annotated)))
        }
    }
}

fn annotate_path(bounds: &VEnv, path: &ast::Path) -> Result<(VEnv, ast::Path), ElabError> {
    let kind = match &path.node.kind {
        ast::PathKind::Root => return Ok((VEnv::new(), path.clone())),
        ast::PathKind::Idx(path_inner, exp) => {
            let (occurs_path, path_inner) = annotate_path(bounds, path_inner)?;
            let (occurs_exp, exp) = annotate_exp(bounds, exp)?;
            let kind = ast::PathKind::Idx(Box::new(path_inner), Box::new(exp));
            (union(occurs_path, occurs_exp)?, kind)
        }
        ast::PathKind::Slice(path_inner, exp_l, exp_h) => {
            let (occurs_path, path_inner) = annotate_path(bounds, path_inner)?;
            let (occurs_l, exp_l) = annotate_exp(bounds, exp_l)?;
            let (occurs_h, exp_h) = annotate_exp(bounds, exp_h)?;
            let kind = ast::PathKind::Slice(Box::new(path_inner), Box::new(exp_l), Box::new(exp_h));
            (union(union(occurs_path, occurs_l)?, occurs_h)?, kind)
        }
        ast::PathKind::Dot(path_inner, atom) => {
            let (occurs, path_inner) = annotate_path(bounds, path_inner)?;
            let kind = ast::PathKind::Dot(Box::new(path_inner), atom.clone());
            (occurs, kind)
        }
    };
    let (occurs, kind) = kind;
    let node = Noted::new(kind, path.node.note.clone());
    Ok((occurs, Spanned::new(node, path.span.clone())))
}

fn annotate_arg(bounds: &VEnv, arg: &ast::Arg) -> Result<(VEnv, ast::Arg), ElabError> {
    match &arg.node {
        ast::ArgKind::Exp(exp) => {
            let (occurs, exp) = annotate_exp(bounds, exp)?;
            Ok((
                occurs,
                Spanned::new(ast::ArgKind::Exp(Box::new(exp)), arg.span.clone()),
            ))
        }
        ast::ArgKind::Def(_) => Ok((VEnv::new(), arg.clone())),
    }
}

fn annotate_args(bounds: &VEnv, args: &[ast::Arg]) -> Result<(VEnv, Vec<ast::Arg>), ElabError> {
    let mut occurs = VEnv::new();
    let mut annotated = Vec::with_capacity(args.len());
    for arg in args {
        let (occurs_arg, arg) = annotate_arg(bounds, arg)?;
        occurs = union(occurs, occurs_arg)?;
        annotated.push(arg);
    }
    Ok((occurs, annotated))
}

fn annotate_prem(bounds: &VEnv, prem: &ast::Prem) -> Result<(VEnv, ast::Prem), ElabError> {
    let (occurs, kind) = match &prem.node {
        ast::PremKind::Rule(rule) => {
            let (occurs, not_exp) = annotate_not_exp(bounds, &rule.not_exp)?;
            let rule = ast::RulePrem {
                id: rule.id.clone(),
                not_exp,
                input_hint: rule.input_hint.clone(),
            };
            (occurs, ast::PremKind::Rule(rule))
        }
        ast::PremKind::If(if_prem) => {
            let (occurs, exp) = annotate_exp(bounds, &if_prem.exp)?;
            (occurs, ast::PremKind::If(ast::IfPrem { exp }))
        }
        ast::PremKind::IfHold(if_prem) => {
            let (occurs, not_exp) = annotate_not_exp(bounds, &if_prem.not_exp)?;
            let if_prem = ast::IfHoldPrem {
                id: if_prem.id.clone(),
                not_exp,
            };
            (occurs, ast::PremKind::IfHold(if_prem))
        }
        ast::PremKind::IfNotHold(if_prem) => {
            let (occurs, not_exp) = annotate_not_exp(bounds, &if_prem.not_exp)?;
            let if_prem = ast::IfNotHoldPrem {
                id: if_prem.id.clone(),
                not_exp,
            };
            (occurs, ast::PremKind::IfNotHold(if_prem))
        }
        ast::PremKind::Let(_) => {
            return Err(ElabError::new(
                ElabErrorKind::MisplacedConstruct,
                prem.span.clone(),
                "let premise should appear only after algorithmic conversion",
            ));
        }
        ast::PremKind::Iter(iterated) => {
            if !iterated.iter_prem.vars_bound.is_empty() || !iterated.iter_prem.vars_bind.is_empty()
            {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIteration,
                    prem.span.clone(),
                    "iterated premise should initially have no annotations",
                ));
            }
            let (occurs, prem_inner) = annotate_prem(bounds, &iterated.prem)?;
            let iter = iterated.iter_prem.iter;
            let vars = collect_iter_vars(bounds, &occurs, iter);
            if vars.is_empty() {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIteration,
                    prem.span.clone(),
                    "empty iteration",
                ));
            }
            let occurs = iterate_occurs(occurs, &vars, iter);
            let iterated = ast::IteratedPrem {
                prem: Box::new(prem_inner),
                iter_prem: ast::IterPrem {
                    iter,
                    vars_bound: vars,
                    vars_bind: vec![],
                },
            };
            (occurs, ast::PremKind::Iter(iterated))
        }
        ast::PremKind::Debug(debug) => {
            let (occurs, exp) = annotate_exp(bounds, &debug.exp)?;
            (occurs, ast::PremKind::Debug(ast::DebugPrem { exp }))
        }
    };
    Ok((occurs, Spanned::new(kind, prem.span.clone())))
}

fn annotate_prems(bounds: &VEnv, prems: &[ast::Prem]) -> Result<(VEnv, Vec<ast::Prem>), ElabError> {
    let mut occurs = VEnv::new();
    let mut annotated = Vec::with_capacity(prems.len());
    for prem in prems {
        let (occurs_prem, prem) = annotate_prem(bounds, prem)?;
        occurs = union(occurs, occurs_prem)?;
        annotated.push(prem);
    }
    Ok((occurs, annotated))
}

fn analyze_rule(rule: &ast::Rule) -> Result<ast::Rule, ElabError> {
    let bounds = infer_rule(rule)?.infer()?;
    let (_, not_exp) = annotate_not_exp(&bounds, &rule.node.not_exp)?;
    let (_, prems) = annotate_prems(&bounds, &rule.node.prems)?;
    let kind = ast::RuleKind {
        id: rule.node.id.clone(),
        not_exp,
        prems,
    };
    Ok(Spanned::new(kind, rule.span.clone()))
}

fn analyze_rule_group(group: &ast::RuleGroup) -> Result<ast::RuleGroup, ElabError> {
    let mut rules = Vec::with_capacity(group.node.1.len());
    for rule in &group.node.1 {
        rules.push(analyze_rule(rule)?);
    }
    Ok(Spanned::new(
        (group.node.0.clone(), rules),
        group.span.clone(),
    ))
}

fn analyze_else_group(group: &ast::ElseGroup) -> Result<ast::ElseGroup, ElabError> {
    Ok(Spanned::new(
        (group.node.0.clone(), analyze_rule(&group.node.1)?),
        group.span.clone(),
    ))
}

fn analyze_clause(clause: &ast::Clause) -> Result<ast::Clause, ElabError> {
    let bounds = infer_clause(clause)?.infer()?;
    let (_, args) = annotate_args(&bounds, &clause.node.args)?;
    let (_, premises) = annotate_prems(&bounds, &clause.node.premises)?;
    let (_, expression) = annotate_exp(&bounds, &clause.node.expression)?;
    let kind = ast::ClauseKind {
        args,
        expression,
        premises,
    };
    Ok(Spanned::new(kind, clause.span.clone()))
}

fn analyze_table_row(row: &ast::TableRow) -> Result<ast::TableRow, ElabError> {
    let bounds = infer_table_row(row)?.infer()?;
    let (_, args) = annotate_args(&bounds, &row.node.0)?;
    let (_, expression) = annotate_exp(&bounds, &row.node.1)?;
    Ok(Spanned::new((args, expression), row.span.clone()))
}

fn analyze_def(def: &ast::Def) -> Result<ast::Def, ElabError> {
    let kind = match &def.node {
        ast::DefKind::Rel(rel) => {
            let mut rule_groups = Vec::with_capacity(rel.rule_groups.len());
            for group in &rel.rule_groups {
                rule_groups.push(analyze_rule_group(group)?);
            }
            let else_group = rel
                .else_group
                .as_ref()
                .map(analyze_else_group)
                .transpose()?;
            ast::DefKind::Rel(ast::Rel {
                id: rel.id.clone(),
                not_typ: rel.not_typ.clone(),
                input_hint: rel.input_hint.clone(),
                rule_groups,
                else_group,
                hints: rel.hints.clone(),
            })
        }
        ast::DefKind::TableDec(table) => {
            let mut rows = Vec::with_capacity(table.rows.len());
            for row in &table.rows {
                rows.push(analyze_table_row(row)?);
            }
            ast::DefKind::TableDec(ast::TableDec {
                id: table.id.clone(),
                params: table.params.clone(),
                typ: table.typ.clone(),
                rows,
                hints: table.hints.clone(),
            })
        }
        ast::DefKind::FuncDec(func) => {
            let mut clauses = Vec::with_capacity(func.clauses.len());
            for clause in &func.clauses {
                clauses.push(analyze_clause(clause)?);
            }
            let else_clause = func.else_clause.as_ref().map(analyze_clause).transpose()?;
            ast::DefKind::FuncDec(ast::FuncDec {
                id: func.id.clone(),
                tparams: func.tparams.clone(),
                params: func.params.clone(),
                typ: func.typ.clone(),
                clauses,
                else_clause,
                hints: func.hints.clone(),
            })
        }
        _ => return Ok(def.clone()),
    };
    Ok(Spanned::new(kind, def.span.clone()))
}

pub(super) fn analyze_spec(spec: &ast::Spec) -> Result<ast::Spec, ElabError> {
    let mut analyzed = Vec::with_capacity(spec.len());
    for def in spec {
        analyzed.push(analyze_def(def)?);
    }
    Ok(analyzed)
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
            hints::input::InputHint,
            il::ast::{self, ExpKind, Iter, RuleKind, TypKind},
        },
        pass::elaborate::ElabErrorKind,
    };

    use super::{analyze_rule, analyze_spec, infer_rule, singleton, union};

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
        let variable_r_span = span("second-variable");
        let variable_r = exp(
            ExpKind::Var(Spanned::new("x".to_owned(), variable_r_span.clone())),
            TypKind::Bool,
            "right-variable",
        );
        let rule = rule(Mixfix::Seq(vec![
            Mixfix::Arg(iter(variable_l, Iter::Opt, "optional")),
            Mixfix::Arg(iter(variable_r, Iter::List, "list")),
        ]));

        let error = infer_rule(&rule).and_then(|dims| dims.infer()).unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::DimensionMismatch);
        assert_eq!(error.span, variable_r_span);
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

    #[test]
    fn annotation_records_each_iteration_variable_at_its_ambient_dimension() {
        let nested = iter(
            iter(variable("x", "variable"), Iter::Opt, "optional"),
            Iter::List,
            "list",
        );

        let analyzed = analyze_rule(&rule(Mixfix::Arg(nested))).expect("annotate rule");
        let Mixfix::Arg(exp_outer) = analyzed.node.not_exp else {
            panic!("argument expression")
        };
        let ExpKind::Iter(exp_inner, (Iter::List, vars_outer)) = exp_outer.node.kind else {
            panic!("outer iteration")
        };
        assert_eq!(vars_outer.len(), 1);
        assert_eq!(vars_outer[0].id.node, "x");
        assert_eq!(vars_outer[0].iters, vec![Iter::Opt]);
        let ExpKind::Iter(_, (Iter::Opt, vars_inner)) = exp_inner.node.kind else {
            panic!("inner iteration")
        };
        assert_eq!(vars_inner.len(), 1);
        assert_eq!(vars_inner[0].id.node, "x");
        assert!(vars_inner[0].iters.is_empty());
    }

    #[test]
    fn equal_dimension_occurrences_keep_the_rightmost_type_span() {
        let id_l = id("x", "left-id");
        let id_r = id("x", "right-id");
        let typ_l = Spanned::new(TypKind::Bool, span("left-type"));
        let typ_r = Spanned::new(TypKind::Bool, span("right-type"));

        let occurs = union(singleton(&id_l, typ_l), singleton(&id_r, typ_r))
            .expect("compatible occurrence types");
        let dim = occurs.get(&id("x", "query")).expect("merged occurrence");

        assert_eq!(dim.typ().span, span("right-type"));
    }

    #[test]
    fn annotation_rejects_empty_iteration_at_the_iteration_span() {
        let iteration_span = span("empty-iteration");
        let literal = exp(ExpKind::Bool(true), TypKind::Bool, "literal");
        let iteration = Spanned::new(
            Noted::new(
                ExpKind::Iter(Box::new(literal), (Iter::List, vec![])),
                TypKind::Iter(
                    Box::new(Spanned::new(TypKind::Bool, span("iter-type"))),
                    Iter::List,
                ),
            ),
            iteration_span.clone(),
        );

        let error = analyze_rule(&rule(Mixfix::Arg(iteration))).unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::InvalidIteration);
        assert_eq!(error.span, iteration_span);
    }

    #[test]
    fn whole_spec_analysis_reaches_rules_nested_in_relation_definitions() {
        let nested = iter(variable("x", "variable"), Iter::List, "list");
        let rule = rule(Mixfix::Arg(nested));
        let group = Spanned::new((id("group", "group"), vec![rule]), span("group"));
        let relation = ast::Rel {
            id: id("relation", "relation"),
            not_typ: Spanned::new(
                Mixfix::Arg(Spanned::new(TypKind::Bool, span("relation-type"))),
                span("relation-type"),
            ),
            input_hint: InputHint::new(vec![0]),
            rule_groups: vec![group],
            else_group: None,
            hints: vec![],
        };
        let spec = vec![Spanned::new(
            ast::DefKind::Rel(relation),
            span("definition"),
        )];

        let analyzed = analyze_spec(&spec).expect("analyze spec");
        let ast::DefKind::Rel(relation) = &analyzed[0].node else {
            panic!("relation definition")
        };
        let Mixfix::Arg(iteration) = &relation.rule_groups[0].node.1[0].node.not_exp else {
            panic!("rule expression")
        };
        let ExpKind::Iter(_, (_, vars)) = &iteration.node.kind else {
            panic!("iterated expression")
        };
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].id.node, "x");
    }
}
