//! Constructors for flat mixfix forms
//!
//! `new` checks the argument count; `fill` supplies each position.

use std::cmp::Ordering;

use super::super::ArityMismatch;
use super::{Mixfix, Mixop, MixopArena};

// - General

/// Pairs a mixop with its arguments.
///
/// Fails unless there is exactly one argument per position;
/// `mixop` must belong to `arena_mixop`.
pub fn new<T>(
    arena_mixop: &MixopArena,
    mixop: Mixop,
    args: Vec<T>,
) -> Result<Mixfix<T>, ArityMismatch> {
    let arity = arena_mixop.arity(mixop);
    match args.len().cmp(&arity) {
        Ordering::Less => Err(ArityMismatch::ArgumentCountTooFew),
        Ordering::Greater => Err(ArityMismatch::ArgumentCountTooMany),
        Ordering::Equal => Ok(Mixfix { mixop, args }),
    }
}

/// Fills each position of a mixop, calling `fill_arg` once per position.
pub fn fill<T>(
    arena_mixop: &MixopArena,
    mixop: Mixop,
    fill_arg: impl FnMut(usize) -> T,
) -> Mixfix<T> {
    let arity = arena_mixop.arity(mixop);
    Mixfix { args: (0..arity).map(fill_arg).collect(), mixop }
}
