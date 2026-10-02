//! Notation of IL and later stages
//!
//! A notation form such as `C |- e : t` is a `Mixfix`:
//! atoms (`|-`, `:`) interleaved with argument holes;
//! `Mixop` is the form without arguments.
//! EL uses only atoms, which stay in `common::notation`.

mod error;
pub mod mixop;
mod tree;

pub use error::ArityMismatch;
pub use mixop::Mixop;
pub use tree::{AtomPhrase, Mixfix};
