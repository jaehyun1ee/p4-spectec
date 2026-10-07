//! Projections that read a mixfix form and its arguments
//!
//! Accessors borrow or consume the parts without changing their pairing.
//! Free identifiers come only from the arguments.

use crate::lang::{common::ds::set::IdSet, traits::free::FreeIds};

use super::Mixfix;

// - Parts

impl<M, T> Mixfix<M, T> {
    /// The form, with argument positions.
    pub fn mixop(&self) -> &M {
        &self.mixop
    }

    /// The arguments in notation order.
    pub fn args(&self) -> &[T] {
        &self.args
    }

    /// The arguments in notation order, for rewriting in place.
    pub fn args_mut(&mut self) -> &mut [T] {
        &mut self.args
    }

    /// The number of argument positions.
    pub fn arity(&self) -> usize {
        self.args.len()
    }

    /// The arguments, dropping the mixop.
    pub fn into_args(self) -> Vec<T> {
        self.args
    }

    /// The mixop and the arguments.
    pub fn into_parts(self) -> (M, Vec<T>) {
        (self.mixop, self.args)
    }
}

// - Free identifiers

impl<M, T: FreeIds> FreeIds for Mixfix<M, T> {
    fn free_ids_into(&self, free: &mut IdSet) {
        self.args.as_slice().free_ids_into(free);
    }
}
