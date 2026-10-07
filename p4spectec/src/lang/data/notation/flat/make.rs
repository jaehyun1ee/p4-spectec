//! Constructors for flat mixfix forms
//!
//! `new` checks the argument count against the arena
//! that issued the mixop handle.

use std::cmp::Ordering;

use super::super::{ArityMismatch, MixopArena, flat};

// - General

impl<T> flat::Mixfix<T> {
    /// Pairs a mixop with its arguments.
    ///
    /// Fails unless there is exactly one argument per position;
    /// `mixop` must belong to `arena_mixop`.
    pub fn new(
        arena_mixop: &MixopArena,
        mixop: flat::Mixop,
        args: Vec<T>,
    ) -> Result<Self, ArityMismatch> {
        match args.len().cmp(&arena_mixop.arity(mixop)) {
            Ordering::Less => Err(ArityMismatch::ArgumentCountTooFew),
            Ordering::Greater => Err(ArityMismatch::ArgumentCountTooMany),
            Ordering::Equal => Ok(Self { mixop, args }),
        }
    }
}
