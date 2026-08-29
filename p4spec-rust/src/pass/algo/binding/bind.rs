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

/// Bindings keyed by source-insensitive identifier identity
pub type Bindings = IdMap<Binding>;

pub fn singleton(id: Id, typ: ast::Typ) -> Bindings {
    let mut bindings = Bindings::new();
    bindings.insert(id, Binding::Single(Dim::new(typ, vec![])));
    bindings
}

pub fn flatten(bindings: &Bindings) -> VEnv {
    bindings
        .iter()
        .map(|(id, binding)| (id.clone(), binding.dim().clone()))
        .collect()
}

pub fn add_iter(bindings: Bindings, iter: ast::Iter) -> Bindings {
    bindings
        .iter()
        .map(|(id, binding)| (id.clone(), binding.clone().add_iter(iter)))
        .collect()
}

/// Combines parallel bindings while retaining the first stored key span
pub fn union(mut bindings_l: Bindings, bindings_r: Bindings) -> Result<Bindings, AlgoError> {
    for (id, binding_r) in bindings_r.iter() {
        let binding_l = bindings_l
            .iter()
            .find(|(stored, _)| stored.node == id.node)
            .map(|(stored, binding)| (stored.span.clone(), binding.clone()));
        let Some((span, binding_l)) = binding_l else {
            bindings_l.insert(id.clone(), binding_r.clone());
            continue;
        };
        if !binding_l.dim().equiv(binding_r.dim()) {
            return Err(AlgoError::new(AlgoErrorKind::InconsistentDimensions, span));
        }
        let dim = binding_l.dim().clone();
        bindings_l.insert(id.clone(), Binding::Multiple(dim));
    }
    Ok(bindings_l)
}
