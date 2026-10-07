//! Notation of IL and later stages, as trees and interned nodes
//!
//! `tree::Mixop` owns an argument-free notation such as `% |- % : %`.
//! `flat::Mixop` refers to a `flat::MixopKind` in a `MixopArena`.
//! `Mixfix` pairs either form with its arguments in notation order.
//! Each representation owns its comparisons and traversals;
//! `print` renders their atoms and argument positions.
//! EL uses only atoms, which stay in `common::notation`.

use crate::lang::common::{notation::atom::Atom, source::Phrase};

mod arena;
mod error;
pub mod external;
pub mod flat;
mod free;
mod get;
mod make;
mod map;
pub mod parse;
pub(crate) mod print;
pub mod tree;
mod view;

pub use arena::MixopArena;
pub use error::{ArityMismatch, MixopError};

// = Notation forms

/// An atom with its source location.
pub type AtomPhrase = Phrase<Atom>;

/// A mixop with one argument per position, in notation order.
///
/// `% |- % : %` with arguments `[C, e, t]` represents `C |- e : t`.
/// Private fields keep the argument count equal to the mixop's arity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mixfix<M, T> {
    /// The form, with argument positions.
    mixop: M,
    /// Arguments in notation order, one per position.
    args: Vec<T>,
}

/// A piece of a notation in reading order.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Piece<'a> {
    /// A literal atom.
    Atom(&'a AtomPhrase),
    /// The argument position with this number.
    Arg(usize),
}
