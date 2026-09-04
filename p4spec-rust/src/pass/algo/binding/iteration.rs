//! Iteration state surrounding a binding operation

use std::ops::{Deref, DerefMut};

use crate::{
    lang::{
        al,
        common::{Id, source::Span},
        il::ast,
    },
    phrase,
    runtime::{dim::Dim, envs::elab::VEnv},
};

use super::super::{AlgoError, AlgoErrorKind};

#[derive(Clone, Debug, PartialEq)]
pub struct Iteration {
    pub iter: ast::Iter,
    pub vars_bound: Vec<ast::Var>,
    pub vars_bind: Vec<ast::Var>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ICtx(Vec<Iteration>);

/// An iteration scope that rolls back its context changes when dropped
pub struct IterationScope<'a> {
    iter_ctx: &'a mut ICtx,
    original: Option<ICtx>,
}

impl Deref for IterationScope<'_> {
    type Target = ICtx;

    fn deref(&self) -> &Self::Target {
        self.iter_ctx
    }
}

impl DerefMut for IterationScope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.iter_ctx
    }
}

impl Drop for IterationScope<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            *self.iter_ctx = original;
        }
    }
}

impl IterationScope<'_> {
    pub fn finish(mut self) -> Iteration {
        let iteration = self.iter_ctx.0.remove(0);
        self.original = None;
        iteration
    }
}

impl ICtx {
    // == Constructors

    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_iterations(iterations: Vec<Iteration>) -> Self {
        Self(iterations)
    }

    pub fn as_slice(&self) -> &[Iteration] {
        &self.0
    }

    pub fn iters(&self) -> Vec<ast::Iter> {
        self.0.iter().map(|entry| entry.iter).collect()
    }

    // == Transactions

    pub fn scope(&mut self, iteration: Iteration) -> IterationScope<'_> {
        let original = self.clone();
        self.0.insert(0, iteration);
        IterationScope {
            iter_ctx: self,
            original: Some(original),
        }
    }

    // == Adders

    fn add_iter(venv: VEnv, iter: ast::Iter) -> VEnv {
        venv.iter()
            .map(|(id, dim)| (id.clone(), dim.clone().add_iter(iter)))
            .collect()
    }

    pub fn add_vars_bound(&mut self, mut venv: VEnv) {
        for entry in &mut self.0 {
            entry
                .vars_bound
                .extend(venv.iter().map(|(id, dim)| ast::Var {
                    id: id.clone(),
                    typ: dim.typ.clone(),
                    iters: dim.iters.clone(),
                }));
            venv = Self::add_iter(venv, entry.iter);
        }
    }

    pub fn add_var_bound(&mut self, id: Id, typ: ast::Typ, iters: Vec<ast::Iter>) {
        let mut venv = VEnv::new();
        venv.insert(id, Dim::new(typ, iters));
        self.add_vars_bound(venv);
    }

    pub fn add_vars_bind(&mut self, mut venv: VEnv) {
        for entry in &mut self.0 {
            entry
                .vars_bind
                .extend(venv.iter().map(|(id, dim)| ast::Var {
                    id: id.clone(),
                    typ: dim.typ.clone(),
                    iters: dim.iters.clone(),
                }));
            venv = Self::add_iter(venv, entry.iter);
        }
    }

    // == Filtering

    pub fn filter_bound(&mut self, mut predicate: impl FnMut(&ast::Var) -> bool) {
        for entry in &mut self.0 {
            entry.vars_bound.retain(&mut predicate);
        }
    }

    // == Validation

    pub fn validate(&self, span: Span) -> Result<(), AlgoError> {
        for entry in &self.0 {
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

    // == Iteration premises

    pub fn iterate_prem(&self, mut prem: al::ast::Prem) -> al::ast::Prem {
        for entry in &self.0 {
            let span = prem.span.clone();
            let prem_iter = al::ast::PremIter {
                iter: entry.iter,
                vars_bound: entry.vars_bound.clone(),
                vars_bind: entry.vars_bind.clone(),
            };
            let iter_prem = al::ast::IterPrem {
                prem: Box::new(prem),
                prem_iter,
            };
            prem = phrase!(node: al::ast::PremKind::Iter(iter_prem), span: span);
        }
        prem
    }
}
