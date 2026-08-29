//! Iteration state surrounding a binding operation

use crate::{
    lang::{
        common::{Id, source::Span},
        il::ast,
    },
    runtime::sta::{Dim, VEnv},
    spanned,
};

use super::super::{AlgoError, AlgoErrorKind};

#[derive(Clone, Debug, PartialEq)]
pub struct Iteration {
    pub iter: ast::Iter,
    pub vars_bound: Vec<ast::Var>,
    pub vars_bind: Vec<ast::Var>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IterationContext {
    iterations: Vec<Iteration>,
}

fn var(id: &Id, dim: &Dim) -> ast::Var {
    ast::Var {
        id: id.clone(),
        typ: dim.typ.clone(),
        iters: dim.iters.clone(),
    }
}

fn add_iter(venv: VEnv, iter: ast::Iter) -> VEnv {
    venv.iter()
        .map(|(id, dim)| (id.clone(), dim.clone().add_iter(iter)))
        .collect()
}

impl IterationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_iterations(iterations: Vec<Iteration>) -> Self {
        Self { iterations }
    }

    pub fn as_slice(&self) -> &[Iteration] {
        &self.iterations
    }

    pub fn iters(&self) -> Vec<ast::Iter> {
        self.iterations.iter().map(|entry| entry.iter).collect()
    }

    pub fn add_vars_bound(&mut self, mut venv: VEnv) {
        for entry in &mut self.iterations {
            entry
                .vars_bound
                .extend(venv.iter().map(|(id, dim)| var(id, dim)));
            venv = add_iter(venv, entry.iter);
        }
    }

    pub fn add_var_bound(&mut self, id: Id, typ: ast::Typ, iters: Vec<ast::Iter>) {
        let mut venv = VEnv::new();
        venv.insert(id, Dim::new(typ, iters));
        self.add_vars_bound(venv);
    }

    pub fn add_vars_bind(&mut self, mut venv: VEnv) {
        for entry in &mut self.iterations {
            entry
                .vars_bind
                .extend(venv.iter().map(|(id, dim)| var(id, dim)));
            venv = add_iter(venv, entry.iter);
        }
    }

    pub fn add_var_bind(&mut self, id: Id, typ: ast::Typ, iters: Vec<ast::Iter>) {
        let mut venv = VEnv::new();
        venv.insert(id, Dim::new(typ, iters));
        self.add_vars_bind(venv);
    }

    pub fn filter_bound(&mut self, mut predicate: impl FnMut(&ast::Var) -> bool) {
        for entry in &mut self.iterations {
            entry.vars_bound.retain(&mut predicate);
        }
    }

    pub fn validate(&self, span: Span) -> Result<(), AlgoError> {
        for entry in &self.iterations {
            if entry.vars_bound.is_empty() {
                let kind = if entry.vars_bind.is_empty() {
                    AlgoErrorKind::EmptyIteration
                } else {
                    AlgoErrorKind::UndeterminedBindingDimension
                };
                return Err(AlgoError::new(kind, span));
            }
        }
        Ok(())
    }

    pub fn iterate_prem(&self, mut prem: ast::Prem) -> ast::Prem {
        for entry in &self.iterations {
            let span = prem.span.clone();
            let iter_prem = ast::IterPrem {
                iter: entry.iter,
                vars_bound: entry.vars_bound.clone(),
                vars_bind: entry.vars_bind.clone(),
            };
            let iterated = ast::IteratedPrem {
                prem: Box::new(prem),
                iter_prem,
            };
            prem = spanned!(node: ast::PremKind::Iter(iterated), span: span);
        }
        prem
    }
}
