//! Projections of generic mixfix forms
//!
//! Accessors borrow or consume the parts without changing their pairing.

use super::Mixfix;

// - Parts

/// The form, with argument positions.
pub fn mixop<M, T>(mixfix: &Mixfix<M, T>) -> &M {
    &mixfix.mixop
}

/// The arguments in notation order.
pub fn args<M, T>(mixfix: &Mixfix<M, T>) -> &[T] {
    &mixfix.args
}

/// The arguments in notation order, for rewriting in place.
pub fn args_mut<M, T>(mixfix: &mut Mixfix<M, T>) -> &mut [T] {
    &mut mixfix.args
}

/// The number of argument positions.
pub fn arity<M, T>(mixfix: &Mixfix<M, T>) -> usize {
    mixfix.args.len()
}

/// The arguments, dropping the mixop.
pub fn into_args<M, T>(mixfix: Mixfix<M, T>) -> Vec<T> {
    mixfix.args
}

/// The mixop and the arguments.
pub fn into_parts<M, T>(mixfix: Mixfix<M, T>) -> (M, Vec<T>) {
    (mixfix.mixop, mixfix.args)
}
