//! Dimension analysis
//!
//! For each rule or clause, collect the dimension of all occurrences of every
//! identifier. The minimal dimension is the ambient dimension of the
//! identifier in the rule or clause.
//!
//! ```text
//! -- if n_x* = [ 1, 1 ] ;; n_x : [ n_x* ]
//! -- if n_y = 1         ;; n_y : [ n_y ]
//! -- if (n_x = n_y)*    ;; n_x : [ n_x* ], n_y : [ n_y* ]
//!
//! Overall, n_x : [ n_x* ] and n_y : [ n_y, n_y* ]
//! Therefore, n_x : n_x* and n_y : n_y
//! ```
//!
//! Annotate iteration constructs with the variables they iterate over.
//!
//! - Variables with iterated dimensions at most the ambient dimension
//! - Check that iteration is non-empty
//!
//! ```text
//! -- if n_x*{n_x <- n_x*} = [ 1, 1 ]
//! -- if n_y = 1
//! -- if (n_x = n_y)*{n_x <- n_x*}
//! ```

use crate::{
    lang::{
        common::{
            Id,
            ds::map::IdMap,
            notation::mixfix::Mixfix,
            source::{Phrase, Span},
        },
        il::ast,
        traits::{eq::SyntaxEq, print::Print},
    },
    phrase,
    runtime::{dim::Dim, envs::elab::VEnv},
};

use super::{ElabError, ElabErrorKind};

// == Dimension inference

// - Context for dimension analysis

type DimPhrase = Phrase<Dim>;

#[derive(Clone, Debug, Default)]
struct DimContext(IdMap<Vec<DimPhrase>>);

impl DimContext {
    // - Occurrence collection

    fn add(&mut self, id: &Id, dim: Dim) {
        let occurrence = phrase!(node: dim, span: id.span.clone());
        if let Some(occurrences) = self.0.get_mut(id) {
            occurrences.push(occurrence);
        } else {
            self.0.insert(id.clone(), vec![occurrence]);
        }
    }

    // - Bound inference

    fn into_bounds(self) -> Result<VEnv, ElabError> {
        let mut bounds = VEnv::new();
        for (id, occurrences) in self.0.iter() {
            let dim_min = occurrences
                .iter()
                .min_by_key(|occurrence| occurrence.node.iters.len())
                .expect("identifier has an occurrence");
            if let Some(dim_conflict) = occurrences
                .iter()
                .find(|occurrence| !dim_min.node.sub(&occurrence.node))
            {
                return Err(ElabError::new(
                    ElabErrorKind::DimensionMismatch,
                    dim_conflict.span.clone(),
                    format!(
                        "mismatched iteration dimensions for identifier `{}`: {} vs {}",
                        id.node,
                        Print::to_string(&dim_min.node),
                        Print::to_string(&dim_conflict.node),
                    ),
                ));
            }
            bounds.insert(id.clone(), dim_min.node.clone());
        }
        Ok(bounds)
    }
}

// - Expression inference

fn infer_exp(dim_ctx: &mut DimContext, exp: &ast::Exp, iters: &[ast::Iter]) {
    match &exp.node {
        ast::ExpKind::Bool(_) | ast::ExpKind::Num(_) | ast::ExpKind::Text(_) => {}
        ast::ExpKind::Var(id) => {
            let typ = phrase!(node: exp.note.as_ref().clone(), span: exp.span.clone());
            dim_ctx.add(id, Dim::new(typ, iters.to_vec()));
        }
        ast::ExpKind::Un(_, _, exp_inner)
        | ast::ExpKind::UpCast(_, exp_inner)
        | ast::ExpKind::DownCast(_, exp_inner)
        | ast::ExpKind::Sub(exp_inner, _, _)
        | ast::ExpKind::Match(exp_inner, _)
        | ast::ExpKind::Len(exp_inner)
        | ast::ExpKind::Dot(exp_inner, _) => infer_exp(dim_ctx, exp_inner, iters),
        ast::ExpKind::Bin(_, _, exp_l, exp_r)
        | ast::ExpKind::Cmp(_, _, exp_l, exp_r)
        | ast::ExpKind::Cons(exp_l, exp_r)
        | ast::ExpKind::Cat(exp_l, exp_r)
        | ast::ExpKind::Mem(exp_l, exp_r)
        | ast::ExpKind::Idx(exp_l, exp_r) => {
            infer_exp(dim_ctx, exp_l, iters);
            infer_exp(dim_ctx, exp_r, iters);
        }
        ast::ExpKind::Tuple(exps) | ast::ExpKind::List(exps) => {
            infer_exps(dim_ctx, exps, iters);
        }
        ast::ExpKind::Case(not_exp) => infer_not_exp(dim_ctx, not_exp, iters),
        ast::ExpKind::Str(fields) => {
            for (_, exp) in fields {
                infer_exp(dim_ctx, exp, iters);
            }
        }
        ast::ExpKind::Opt(exp_inner) => {
            if let Some(exp_inner) = exp_inner {
                infer_exp(dim_ctx, exp_inner, iters);
            }
        }
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            infer_exp(dim_ctx, exp_base, iters);
            infer_exp(dim_ctx, exp_l, iters);
            infer_exp(dim_ctx, exp_h, iters);
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            infer_exp(dim_ctx, exp_base, iters);
            infer_path(dim_ctx, path, iters);
            infer_exp(dim_ctx, exp_field, iters);
        }
        ast::ExpKind::Call(_, _, args) => infer_args(dim_ctx, args, iters),
        ast::ExpKind::Iter(exp_inner, (iter, _)) => {
            let mut iters_inner = Vec::with_capacity(iters.len() + 1);
            iters_inner.push(*iter);
            iters_inner.extend_from_slice(iters);
            infer_exp(dim_ctx, exp_inner, &iters_inner);
        }
    }
}

fn infer_exps(dim_ctx: &mut DimContext, exps: &[ast::Exp], iters: &[ast::Iter]) {
    for exp in exps {
        infer_exp(dim_ctx, exp, iters);
    }
}

// - Notation expression inference

fn infer_not_exp(dim_ctx: &mut DimContext, not_exp: &ast::NotExp, iters: &[ast::Iter]) {
    not_exp.iter(|exp| infer_exp(dim_ctx, exp, iters));
}

// - Path inference

fn infer_path(dim_ctx: &mut DimContext, path: &ast::Path, iters: &[ast::Iter]) {
    match &path.node {
        ast::PathKind::Root => {}
        ast::PathKind::Idx(path_inner, exp) => {
            infer_path(dim_ctx, path_inner, iters);
            infer_exp(dim_ctx, exp, iters);
        }
        ast::PathKind::Slice(path_inner, exp_l, exp_h) => {
            infer_path(dim_ctx, path_inner, iters);
            infer_exp(dim_ctx, exp_l, iters);
            infer_exp(dim_ctx, exp_h, iters);
        }
        ast::PathKind::Dot(path_inner, _) => infer_path(dim_ctx, path_inner, iters),
    }
}

// - Argument inference

fn infer_arg(dim_ctx: &mut DimContext, arg: &ast::Arg, iters: &[ast::Iter]) {
    if let ast::ArgKind::Exp(exp) = &arg.node {
        infer_exp(dim_ctx, exp, iters);
    }
}

fn infer_args(dim_ctx: &mut DimContext, args: &[ast::Arg], iters: &[ast::Iter]) {
    for arg in args {
        infer_arg(dim_ctx, arg, iters);
    }
}

// - Premise inference

fn infer_prem(
    dim_ctx: &mut DimContext,
    prem: &ast::Prem,
    iters: &[ast::Iter],
) -> Result<(), ElabError> {
    match &prem.node {
        ast::PremKind::Rule(rule) => infer_not_exp(dim_ctx, &rule.not_exp, iters),
        ast::PremKind::If(if_prem) => infer_exp(dim_ctx, &if_prem.exp, iters),
        ast::PremKind::IfHold(if_prem) => infer_not_exp(dim_ctx, &if_prem.not_exp, iters),
        ast::PremKind::IfNotHold(if_prem) => infer_not_exp(dim_ctx, &if_prem.not_exp, iters),
        ast::PremKind::Iter(iter_prem) => {
            if !iter_prem.prem_iter.vars_bound.is_empty()
                || !iter_prem.prem_iter.vars_bind.is_empty()
            {
                return Err(ElabError::new(
                    ElabErrorKind::InvalidIteration,
                    prem.span.clone(),
                    "iterated premise should initially have no annotations",
                ));
            }
            let mut iters_inner = Vec::with_capacity(iters.len() + 1);
            iters_inner.push(iter_prem.prem_iter.iter);
            iters_inner.extend_from_slice(iters);
            infer_prem(dim_ctx, &iter_prem.prem, &iters_inner)?;
        }
        ast::PremKind::Debug(debug) => infer_exp(dim_ctx, &debug.exp, iters),
    }
    Ok(())
}

fn infer_prems(dim_ctx: &mut DimContext, prems: &[ast::Prem]) -> Result<(), ElabError> {
    for prem in prems {
        infer_prem(dim_ctx, prem, &[])?;
    }
    Ok(())
}

// - Rule inference

fn infer_rule(rule: &ast::Rule) -> Result<DimContext, ElabError> {
    let mut dim_ctx = DimContext::default();
    infer_not_exp(&mut dim_ctx, &rule.node.not_exp, &[]);
    infer_prems(&mut dim_ctx, &rule.node.prems)?;
    Ok(dim_ctx)
}

// - Clause inference

fn infer_clause(clause: &ast::Clause) -> Result<DimContext, ElabError> {
    let mut dim_ctx = DimContext::default();
    infer_args(&mut dim_ctx, &clause.node.args, &[]);
    infer_prems(&mut dim_ctx, &clause.node.premises)?;
    infer_exp(&mut dim_ctx, &clause.node.expression, &[]);
    Ok(dim_ctx)
}

// - Table row inference

fn infer_table_row(row: &ast::TableRow) -> DimContext {
    let mut dim_ctx = DimContext::default();
    infer_args(&mut dim_ctx, &row.node.0, &[]);
    infer_exp(&mut dim_ctx, &row.node.1, &[]);
    dim_ctx
}

// == Dimension annotation

// - Occurrences

struct Occurrences(VEnv);

impl Occurrences {
    fn new() -> Self {
        Self(VEnv::new())
    }

    fn singleton(id: &Id, typ: ast::Typ) -> Self {
        let mut occurs = VEnv::new();
        occurs.insert(id.clone(), Dim::new(typ, vec![]));
        Self(occurs)
    }

    fn union(mut self, occurs_other: Self) -> Result<Self, ElabError> {
        for (id, dim_other) in occurs_other.0.iter() {
            if let Some(dim) = self.0.get(id) {
                if !dim.typ.syntax_eq(&dim_other.typ) {
                    return Err(ElabError::new(
                        ElabErrorKind::TypeMismatch,
                        id.span.clone(),
                        format!(
                            "type mismatch for identifier `{}` in union: {} vs {}",
                            id.node,
                            Print::to_string(&dim.typ),
                            Print::to_string(&dim_other.typ),
                        ),
                    ));
                }
                if dim_other.iters.len() <= dim.iters.len() {
                    self.0.insert(id.clone(), dim_other.clone());
                }
            } else {
                self.0.insert(id.clone(), dim_other.clone());
            }
        }
        Ok(self)
    }

    fn iterate(mut self, vars: &[ast::Var], iter: ast::Iter) -> Self {
        for var in vars {
            let mut iters = var.iters.clone();
            iters.push(iter);
            self.0
                .insert(var.id.clone(), Dim::new(var.typ.clone(), iters));
        }
        self
    }

    fn iter(&self) -> impl Iterator<Item = (&Id, &Dim)> {
        self.0.iter()
    }
}

// - Iteration variables

fn collect_iter_vars(bounds: &VEnv, occurs: &Occurrences, iter: ast::Iter) -> Vec<ast::Var> {
    occurs
        .iter()
        .filter_map(|(id, dim)| {
            let dim_bound = bounds
                .get(id)
                .expect("occurring variable has inferred bound");
            if dim.clone().add_iter(iter).sub(dim_bound) {
                Some(ast::Var {
                    id: id.clone(),
                    typ: dim.typ.clone(),
                    iters: dim.iters.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

// - Expression annotation

fn annotate_exp(bounds: &VEnv, exp: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    let span = &exp.span;
    let typ_kind = exp.note.as_ref();
    match &mut exp.node {
        ast::ExpKind::Bool(_) => Ok(annotate_bool_exp()),
        ast::ExpKind::Num(_) => Ok(annotate_num_exp()),
        ast::ExpKind::Text(_) => Ok(annotate_text_exp()),
        ast::ExpKind::Var(id) => Ok(annotate_var_exp(span, typ_kind, id)),
        ast::ExpKind::Un(_, _, exp_inner) => annotate_un_exp(bounds, exp_inner),
        ast::ExpKind::Bin(_, _, exp_l, exp_r) => annotate_bin_exp(bounds, exp_l, exp_r),
        ast::ExpKind::Cmp(_, _, exp_l, exp_r) => annotate_cmp_exp(bounds, exp_l, exp_r),
        ast::ExpKind::UpCast(_, exp_inner) => annotate_upcast_exp(bounds, exp_inner),
        ast::ExpKind::DownCast(_, exp_inner) => annotate_downcast_exp(bounds, exp_inner),
        ast::ExpKind::Sub(exp_inner, _, _) => annotate_sub_exp(bounds, exp_inner),
        ast::ExpKind::Match(exp_inner, _) => annotate_match_exp(bounds, exp_inner),
        ast::ExpKind::Tuple(exps) => annotate_tuple_exp(bounds, exps),
        ast::ExpKind::Case(not_exp) => annotate_case_exp(bounds, not_exp),
        ast::ExpKind::Str(fields) => annotate_str_exp(bounds, fields),
        ast::ExpKind::Opt(exp_inner) => annotate_opt_exp(bounds, exp_inner.as_deref_mut()),
        ast::ExpKind::List(exps) => annotate_list_exp(bounds, exps),
        ast::ExpKind::Cons(exp_l, exp_r) => annotate_cons_exp(bounds, exp_l, exp_r),
        ast::ExpKind::Cat(exp_l, exp_r) => annotate_cat_exp(bounds, exp_l, exp_r),
        ast::ExpKind::Mem(exp_l, exp_r) => annotate_mem_exp(bounds, exp_l, exp_r),
        ast::ExpKind::Len(exp_inner) => annotate_len_exp(bounds, exp_inner),
        ast::ExpKind::Dot(exp_inner, _) => annotate_dot_exp(bounds, exp_inner),
        ast::ExpKind::Idx(exp_l, exp_r) => annotate_idx_exp(bounds, exp_l, exp_r),
        ast::ExpKind::Slice(exp_base, exp_l, exp_h) => {
            annotate_slice_exp(bounds, exp_base, exp_l, exp_h)
        }
        ast::ExpKind::Upd(exp_base, path, exp_field) => {
            annotate_upd_exp(bounds, exp_base, path, exp_field)
        }
        ast::ExpKind::Call(_, _, args) => annotate_call_exp(bounds, args),
        ast::ExpKind::Iter(exp_inner, (iter, vars)) => {
            annotate_iter_exp(bounds, span, exp_inner, *iter, vars)
        }
    }
}

fn annotate_exps(bounds: &VEnv, exps: &mut [ast::Exp]) -> Result<Occurrences, ElabError> {
    let mut occurs = Occurrences::new();
    for exp in exps {
        let occurs_exp = annotate_exp(bounds, exp)?;
        occurs = occurs.union(occurs_exp)?;
    }
    Ok(occurs)
}

// - Boolean expressions

fn annotate_bool_exp() -> Occurrences {
    Occurrences::new()
}

// - Numeric expressions

fn annotate_num_exp() -> Occurrences {
    Occurrences::new()
}

// - Text expressions

fn annotate_text_exp() -> Occurrences {
    Occurrences::new()
}

// - Variable expressions

fn annotate_var_exp(span: &Span, typ_kind: &ast::TypKind, id: &Id) -> Occurrences {
    let typ = phrase!(node: typ_kind.clone(), span: span.clone());
    Occurrences::singleton(id, typ)
}

// - Unary expressions

fn annotate_un_exp(bounds: &VEnv, exp_inner: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Binary expressions

fn annotate_bin_exp(
    bounds: &VEnv,
    exp_l: &mut ast::Exp,
    exp_r: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_r = annotate_exp(bounds, exp_r)?;
    occurs_l.union(occurs_r)
}

// - Comparison expressions

fn annotate_cmp_exp(
    bounds: &VEnv,
    exp_l: &mut ast::Exp,
    exp_r: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_r = annotate_exp(bounds, exp_r)?;
    occurs_l.union(occurs_r)
}

// - Upcast expressions

fn annotate_upcast_exp(bounds: &VEnv, exp_inner: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Downcast expressions

fn annotate_downcast_exp(
    bounds: &VEnv,
    exp_inner: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Subtype expressions

fn annotate_sub_exp(bounds: &VEnv, exp_inner: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Match expressions

fn annotate_match_exp(bounds: &VEnv, exp_inner: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Tuple expressions

fn annotate_tuple_exp(bounds: &VEnv, exps: &mut [ast::Exp]) -> Result<Occurrences, ElabError> {
    annotate_exps(bounds, exps)
}

// - Case expressions

fn annotate_case_exp(bounds: &VEnv, not_exp: &mut ast::NotExp) -> Result<Occurrences, ElabError> {
    annotate_not_exp(bounds, not_exp)
}

// - Struct expressions

fn annotate_str_exp(
    bounds: &VEnv,
    fields: &mut [(ast::Atom, ast::Exp)],
) -> Result<Occurrences, ElabError> {
    let mut occurs = Occurrences::new();
    for (_, exp) in fields {
        let occurs_exp = annotate_exp(bounds, exp)?;
        occurs = occurs.union(occurs_exp)?;
    }
    Ok(occurs)
}

// - Optional expressions

fn annotate_opt_exp(
    bounds: &VEnv,
    exp_inner: Option<&mut ast::Exp>,
) -> Result<Occurrences, ElabError> {
    exp_inner.map_or_else(|| Ok(Occurrences::new()), |exp| annotate_exp(bounds, exp))
}

// - List expressions

fn annotate_list_exp(bounds: &VEnv, exps: &mut [ast::Exp]) -> Result<Occurrences, ElabError> {
    annotate_exps(bounds, exps)
}

// - List construction expressions

fn annotate_cons_exp(
    bounds: &VEnv,
    exp_l: &mut ast::Exp,
    exp_r: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_r = annotate_exp(bounds, exp_r)?;
    occurs_l.union(occurs_r)
}

// - Concatenation expressions

fn annotate_cat_exp(
    bounds: &VEnv,
    exp_l: &mut ast::Exp,
    exp_r: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_r = annotate_exp(bounds, exp_r)?;
    occurs_l.union(occurs_r)
}

// - Membership expressions

fn annotate_mem_exp(
    bounds: &VEnv,
    exp_l: &mut ast::Exp,
    exp_r: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_r = annotate_exp(bounds, exp_r)?;
    occurs_l.union(occurs_r)
}

// - Length expressions

fn annotate_len_exp(bounds: &VEnv, exp_inner: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Field access expressions

fn annotate_dot_exp(bounds: &VEnv, exp_inner: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp_inner)
}

// - Index expressions

fn annotate_idx_exp(
    bounds: &VEnv,
    exp_l: &mut ast::Exp,
    exp_r: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_r = annotate_exp(bounds, exp_r)?;
    occurs_l.union(occurs_r)
}

// - Slice expressions

fn annotate_slice_exp(
    bounds: &VEnv,
    exp_base: &mut ast::Exp,
    exp_l: &mut ast::Exp,
    exp_h: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_base = annotate_exp(bounds, exp_base)?;
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_h = annotate_exp(bounds, exp_h)?;
    let occurs = occurs_base.union(occurs_l)?;
    occurs.union(occurs_h)
}

// - Update expressions

fn annotate_upd_exp(
    bounds: &VEnv,
    exp_base: &mut ast::Exp,
    path: &mut ast::Path,
    exp_field: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_base = annotate_exp(bounds, exp_base)?;
    let occurs_field = annotate_exp(bounds, exp_field)?;
    let occurs_path = annotate_path(bounds, path)?;
    let occurs = occurs_base.union(occurs_field)?;
    occurs.union(occurs_path)
}

// - Call expressions

fn annotate_call_exp(bounds: &VEnv, args: &mut [ast::Arg]) -> Result<Occurrences, ElabError> {
    annotate_args(bounds, args)
}

// - Iteration expressions

fn annotate_iter_exp(
    bounds: &VEnv,
    span: &Span,
    exp_inner: &mut ast::Exp,
    iter: ast::Iter,
    vars: &mut Vec<ast::Var>,
) -> Result<Occurrences, ElabError> {
    if !vars.is_empty() {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIteration,
            span.clone(),
            "iterated expression should initially have no annotations",
        ));
    }
    let occurs = annotate_exp(bounds, exp_inner)?;
    let vars_inner = collect_iter_vars(bounds, &occurs, iter);
    if vars_inner.is_empty() {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIteration,
            span.clone(),
            "empty iteration",
        ));
    }
    let occurs = occurs.iterate(&vars_inner, iter);
    *vars = vars_inner;
    Ok(occurs)
}

// - Notation expression annotation

fn annotate_not_exp(bounds: &VEnv, not_exp: &mut ast::NotExp) -> Result<Occurrences, ElabError> {
    match not_exp {
        Mixfix::Arg(exp) => annotate_arg_not_exp(bounds, exp),
        Mixfix::Atom(_) => Ok(annotate_atom_not_exp()),
        Mixfix::Brack(_, not_exp_inner, _) => annotate_brack_not_exp(bounds, not_exp_inner),
        Mixfix::Infix(not_exp_l, _, not_exp_r) => {
            annotate_infix_not_exp(bounds, not_exp_l, not_exp_r)
        }
        Mixfix::Seq(not_exps) => annotate_seq_not_exp(bounds, not_exps),
    }
}

// - Notation arguments

fn annotate_arg_not_exp(bounds: &VEnv, exp: &mut ast::Exp) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, exp)
}

// - Notation atoms

fn annotate_atom_not_exp() -> Occurrences {
    Occurrences::new()
}

// - Bracketed notation expressions

fn annotate_brack_not_exp(
    bounds: &VEnv,
    not_exp_inner: &mut ast::NotExp,
) -> Result<Occurrences, ElabError> {
    annotate_not_exp(bounds, not_exp_inner)
}

// - Infix notation expressions

fn annotate_infix_not_exp(
    bounds: &VEnv,
    not_exp_l: &mut ast::NotExp,
    not_exp_r: &mut ast::NotExp,
) -> Result<Occurrences, ElabError> {
    let occurs_l = annotate_not_exp(bounds, not_exp_l)?;
    let occurs_r = annotate_not_exp(bounds, not_exp_r)?;
    occurs_l.union(occurs_r)
}

// - Notation sequences

fn annotate_seq_not_exp(
    bounds: &VEnv,
    not_exps: &mut [ast::NotExp],
) -> Result<Occurrences, ElabError> {
    let mut occurs = Occurrences::new();
    for not_exp in not_exps {
        let occurs_exp = annotate_not_exp(bounds, not_exp)?;
        occurs = occurs.union(occurs_exp)?;
    }
    Ok(occurs)
}

// - Path annotation

fn annotate_path(bounds: &VEnv, path: &mut ast::Path) -> Result<Occurrences, ElabError> {
    match &mut path.node {
        ast::PathKind::Root => Ok(annotate_root_path()),
        ast::PathKind::Idx(path_inner, exp) => annotate_idx_path(bounds, path_inner, exp),
        ast::PathKind::Slice(path_inner, exp_l, exp_h) => {
            annotate_slice_path(bounds, path_inner, exp_l, exp_h)
        }
        ast::PathKind::Dot(path_inner, _) => annotate_dot_path(bounds, path_inner),
    }
}

// - Root paths

fn annotate_root_path() -> Occurrences {
    Occurrences::new()
}

// - Index paths

fn annotate_idx_path(
    bounds: &VEnv,
    path_inner: &mut ast::Path,
    exp: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_path = annotate_path(bounds, path_inner)?;
    let occurs_exp = annotate_exp(bounds, exp)?;
    occurs_path.union(occurs_exp)
}

// - Slice paths

fn annotate_slice_path(
    bounds: &VEnv,
    path_inner: &mut ast::Path,
    exp_l: &mut ast::Exp,
    exp_h: &mut ast::Exp,
) -> Result<Occurrences, ElabError> {
    let occurs_path = annotate_path(bounds, path_inner)?;
    let occurs_l = annotate_exp(bounds, exp_l)?;
    let occurs_h = annotate_exp(bounds, exp_h)?;
    let occurs = occurs_path.union(occurs_l)?;
    occurs.union(occurs_h)
}

// - Field paths

fn annotate_dot_path(bounds: &VEnv, path_inner: &mut ast::Path) -> Result<Occurrences, ElabError> {
    annotate_path(bounds, path_inner)
}

// - Argument annotation

fn annotate_arg(bounds: &VEnv, arg: &mut ast::Arg) -> Result<Occurrences, ElabError> {
    match &mut arg.node {
        ast::ArgKind::Exp(exp) => annotate_exp(bounds, exp),
        ast::ArgKind::Def(_) => Ok(Occurrences::new()),
    }
}

fn annotate_args(bounds: &VEnv, args: &mut [ast::Arg]) -> Result<Occurrences, ElabError> {
    let mut occurs = Occurrences::new();
    for arg in args {
        let occurs_arg = annotate_arg(bounds, arg)?;
        occurs = occurs.union(occurs_arg)?;
    }
    Ok(occurs)
}

// - Premise annotation

fn annotate_prem(bounds: &VEnv, prem: &mut ast::Prem) -> Result<Occurrences, ElabError> {
    let span = &prem.span;
    match &mut prem.node {
        ast::PremKind::Rule(rule) => annotate_rule_prem(bounds, rule),
        ast::PremKind::If(if_prem) => annotate_if_prem(bounds, if_prem),
        ast::PremKind::IfHold(if_prem) => annotate_if_hold_prem(bounds, if_prem),
        ast::PremKind::IfNotHold(if_prem) => annotate_if_not_hold_prem(bounds, if_prem),
        ast::PremKind::Iter(iter_prem) => annotate_iter_prem(bounds, span, iter_prem),
        ast::PremKind::Debug(debug) => annotate_debug_prem(bounds, debug),
    }
}

fn annotate_prems(bounds: &VEnv, prems: &mut [ast::Prem]) -> Result<Occurrences, ElabError> {
    let mut occurs = Occurrences::new();
    for prem in prems {
        let occurs_prem = annotate_prem(bounds, prem)?;
        occurs = occurs.union(occurs_prem)?;
    }
    Ok(occurs)
}

// - Rule premises

fn annotate_rule_prem(bounds: &VEnv, rule: &mut ast::RulePrem) -> Result<Occurrences, ElabError> {
    annotate_not_exp(bounds, &mut rule.not_exp)
}

// - Conditional premises

fn annotate_if_prem(bounds: &VEnv, if_prem: &mut ast::IfPrem) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, &mut if_prem.exp)
}

// - Holding premises

fn annotate_if_hold_prem(
    bounds: &VEnv,
    if_prem: &mut ast::IfHoldPrem,
) -> Result<Occurrences, ElabError> {
    annotate_not_exp(bounds, &mut if_prem.not_exp)
}

// - Non-holding premises

fn annotate_if_not_hold_prem(
    bounds: &VEnv,
    if_prem: &mut ast::IfNotHoldPrem,
) -> Result<Occurrences, ElabError> {
    annotate_not_exp(bounds, &mut if_prem.not_exp)
}

// - Iteration premises

fn annotate_iter_prem(
    bounds: &VEnv,
    span: &Span,
    iter_prem: &mut ast::IterPrem,
) -> Result<Occurrences, ElabError> {
    if !iter_prem.prem_iter.vars_bound.is_empty() || !iter_prem.prem_iter.vars_bind.is_empty() {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIteration,
            span.clone(),
            "iterated premise should initially have no annotations",
        ));
    }
    let occurs = annotate_prem(bounds, &mut iter_prem.prem)?;
    let iter = iter_prem.prem_iter.iter;
    let vars_bound = collect_iter_vars(bounds, &occurs, iter);
    if vars_bound.is_empty() {
        return Err(ElabError::new(
            ElabErrorKind::InvalidIteration,
            span.clone(),
            "empty iteration",
        ));
    }
    let occurs = occurs.iterate(&vars_bound, iter);
    iter_prem.prem_iter.vars_bound = vars_bound;
    Ok(occurs)
}

// - Debug premises

fn annotate_debug_prem(
    bounds: &VEnv,
    debug: &mut ast::DebugPrem,
) -> Result<Occurrences, ElabError> {
    annotate_exp(bounds, &mut debug.exp)
}

// == Analysis

// - Rules

fn analyze_rule(rule: &mut ast::Rule) -> Result<(), ElabError> {
    let bounds = infer_rule(rule)?.into_bounds()?;
    annotate_not_exp(&bounds, &mut rule.node.not_exp)?;
    annotate_prems(&bounds, &mut rule.node.prems)?;
    Ok(())
}

// - Rule groups

fn analyze_rule_group(group: &mut ast::RuleGroup) -> Result<(), ElabError> {
    for rule in &mut group.node.1 {
        analyze_rule(rule)?;
    }
    Ok(())
}

// - Otherwise groups

fn analyze_else_group(group: &mut ast::ElseGroup) -> Result<(), ElabError> {
    analyze_rule(&mut group.node.1)
}

// - Clauses

fn analyze_clause(clause: &mut ast::Clause) -> Result<(), ElabError> {
    let bounds = infer_clause(clause)?.into_bounds()?;
    annotate_args(&bounds, &mut clause.node.args)?;
    annotate_prems(&bounds, &mut clause.node.premises)?;
    annotate_exp(&bounds, &mut clause.node.expression)?;
    Ok(())
}

// - Table rows

fn analyze_table_row(row: &mut ast::TableRow) -> Result<(), ElabError> {
    let bounds = infer_table_row(row).into_bounds()?;
    annotate_args(&bounds, &mut row.node.0)?;
    annotate_exp(&bounds, &mut row.node.1)?;
    Ok(())
}

// - Definitions

fn analyze_def(def: &mut ast::Def) -> Result<(), ElabError> {
    match &mut def.node {
        ast::DefKind::Rel(rel) => analyze_rel_def(rel),
        ast::DefKind::TableDec(table) => analyze_table_def(table),
        ast::DefKind::FuncDec(func) => analyze_func_def(func),
        _ => Ok(()),
    }
}

// - Relation definitions

fn analyze_rel_def(rel: &mut ast::Rel) -> Result<(), ElabError> {
    for group in &mut rel.rule_groups {
        analyze_rule_group(group)?;
    }
    if let Some(group) = &mut rel.else_group {
        analyze_else_group(group)?;
    }
    Ok(())
}

// - Table definitions

fn analyze_table_def(table: &mut ast::TableDec) -> Result<(), ElabError> {
    for row in &mut table.rows {
        analyze_table_row(row)?;
    }
    Ok(())
}

// - Function definitions

fn analyze_func_def(func: &mut ast::FuncDec) -> Result<(), ElabError> {
    for clause in &mut func.clauses {
        analyze_clause(clause)?;
    }
    if let Some(clause) = &mut func.else_clause {
        analyze_clause(clause)?;
    }
    Ok(())
}

// - Specification

pub(super) fn analyze_spec(spec: &mut ast::Spec) -> Result<(), ElabError> {
    for def in spec {
        analyze_def(def)?;
    }
    Ok(())
}
