//! Notation of IL and later stages, as trees and interned nodes
//!
//! `tree::Mixop` owns an argument-free notation such as `% |- % : %`.
//! `flat::Mixop` refers to a `flat::MixopKind` in a `MixopArena`.
//! `Mixfix` pairs either form with its arguments in notation order.
//! Each representation owns its comparisons and traversals;
//! `print` renders their atoms and argument positions.
//! EL uses only atoms, which stay in `common::notation`.

mod arena;
mod error;
pub mod external;
pub mod flat;
mod mixfix;
pub mod mixop;
pub(crate) mod print;
pub mod tree;

pub use arena::MixopArena;
pub use error::{ArityMismatch, MixopError};
pub use mixfix::Mixfix;
/// An atom with its source location.
pub type AtomPhrase =
    crate::lang::common::source::Phrase<crate::lang::common::notation::atom::Atom>;

/// A piece of a notation in reading order.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Piece<'a> {
    /// A literal atom.
    Atom(&'a AtomPhrase),
    /// The argument position with this number.
    Arg(usize),
}
