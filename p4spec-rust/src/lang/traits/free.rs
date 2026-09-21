//! Free identifiers shared across language stages

use std::rc::Rc;

use crate::lang::{
    common::{ds::set::IdSet, source::NotePhrase},
    il::ast::Var,
    traits::eq::SyntaxEq,
};

// == Free identifiers

/// Collects free term identifiers from syntax
pub trait FreeIds {
    /// Returns the free term identifiers contained in `self`
    fn free_ids(&self) -> IdSet {
        let mut free = IdSet::new();
        self.free_ids_into(&mut free);
        free
    }

    /// Adds the free term identifiers contained in `self` to `free`
    fn free_ids_into(&self, free: &mut IdSet) {
        free.append(self.free_ids());
    }
}

// == Free variables

/// Collects dimension-aware free variables from syntax.
pub trait FreeVars {
    /// Returns the free variables contained in `self`.
    fn free_vars(&self) -> Vec<Var>;

    /// Adds the free variables contained in `self` to `vars_free`.
    fn free_vars_into(&self, vars_free: &mut Vec<Var>) {
        for var in self.free_vars() {
            if !vars_free.iter().any(|var_free| var_free.syntax_eq(&var)) {
                vars_free.push(var);
            }
        }
    }
}

// - Text

impl FreeIds for String {
    fn free_ids(&self) -> IdSet {
        IdSet::new()
    }
}

// - Source annotations

impl<T: FreeIds, N, S> FreeIds for NotePhrase<T, N, S> {
    fn free_ids_into(&self, free: &mut IdSet) {
        self.node.free_ids_into(free);
    }
}

// - Containers

impl<T: FreeIds + ?Sized> FreeIds for Box<T> {
    fn free_ids_into(&self, free: &mut IdSet) {
        self.as_ref().free_ids_into(free);
    }
}

impl<T: FreeIds + ?Sized> FreeIds for Rc<T> {
    fn free_ids_into(&self, free: &mut IdSet) {
        self.as_ref().free_ids_into(free);
    }
}

impl<T: FreeIds> FreeIds for Option<T> {
    fn free_ids_into(&self, free: &mut IdSet) {
        if let Some(value) = self {
            value.free_ids_into(free);
        }
    }
}

impl<T: FreeIds> FreeIds for [T] {
    fn free_ids_into(&self, free: &mut IdSet) {
        for item in self {
            item.free_ids_into(free);
        }
    }
}
