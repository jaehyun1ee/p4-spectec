//! Singular and repeated identifier bindings

use crate::{
    lang::{
        common::{Id, ds::map::IdMap},
        il::ast,
    },
    runtime::sta::{Dim, VEnv},
};

use super::super::{AlgoError, AlgoErrorKind};

/// One binding occurrence or multiple parallel occurrences
#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Single(Dim),
    Multiple(Dim),
}

impl Binding {
    pub fn dim(&self) -> &Dim {
        match self {
            Self::Single(dim) | Self::Multiple(dim) => dim,
        }
    }

    pub fn add_iter(self, iter: ast::Iter) -> Self {
        match self {
            Self::Single(dim) => Self::Single(dim.add_iter(iter)),
            Self::Multiple(dim) => Self::Multiple(dim.add_iter(iter)),
        }
    }
}

/// Binding environment keyed by source-insensitive identifier identity
#[derive(Clone, Debug)]
pub struct BEnv(IdMap<Binding>);

impl BEnv {
    pub fn new() -> Self {
        Self(IdMap::new())
    }

    pub fn singleton(id: Id, typ: ast::Typ) -> Self {
        let mut benv = Self::new();
        benv.insert(id, Binding::Single(Dim::new(typ, vec![])));
        benv
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn insert(&mut self, id: Id, binding: Binding) {
        self.0.insert(id, binding);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Id, &Binding)> {
        self.0.iter()
    }

    pub fn flatten(&self) -> VEnv {
        self.iter()
            .map(|(id, binding)| (id.clone(), binding.dim().clone()))
            .collect()
    }

    pub fn add_iter(self, iter: ast::Iter) -> Self {
        let entries = self
            .iter()
            .map(|(id, binding)| (id.clone(), binding.clone().add_iter(iter)))
            .collect();
        Self(entries)
    }

    /// Combines parallel bindings while retaining the first stored key span
    pub fn union(mut self, other: Self) -> Result<Self, AlgoError> {
        for (id, binding_r) in other.iter() {
            let binding_l = self
                .iter()
                .find(|(stored, _)| stored.node == id.node)
                .map(|(stored, binding)| (stored.span.clone(), binding.clone()));
            let Some((span, binding_l)) = binding_l else {
                self.insert(id.clone(), binding_r.clone());
                continue;
            };
            let dim_l = binding_l.dim();
            let dim_r = binding_r.dim();
            if !(dim_l.sub(dim_r) && dim_r.sub(dim_l)) {
                return Err(AlgoError::new(AlgoErrorKind::InconsistentDimensions, span));
            }
            let dim = dim_l.clone();
            self.insert(id.clone(), Binding::Multiple(dim));
        }
        Ok(self)
    }
}
