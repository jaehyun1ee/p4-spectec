//! Notation of IL and later stages, as trees and interned nodes
//!
//! `tree::Mixop` owns an argument-free notation such as `% |- % : %`.
//! `flat::Mixop` stores one node with child handles in a `MixopArena`.
//! `Mixfix` pairs either form with its arguments in notation order.
//! `walk` provides concrete traversals for both representations.
//! EL uses only atoms, which stay in `common::notation`.

mod arena;
mod error;
pub mod external;
pub mod flat;
mod mixfix;
pub mod mixop;
pub mod tree;
pub mod walk;

pub use arena::MixopArena;
pub use error::{ArityMismatch, MixopError};
pub use flat::MixopId;
pub use mixfix::{Mixfix, MixfixRef, View};
pub use mixop::MixopMatch;
/// An atom with its source location.
pub type AtomPhrase =
    crate::lang::common::source::Phrase<crate::lang::common::notation::atom::Atom>;
