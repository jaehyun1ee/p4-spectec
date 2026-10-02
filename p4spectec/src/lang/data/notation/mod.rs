//! Notation of IL and later stages
//!
//! A notation form such as `C |- e : t` is a `Mixfix`:
//! atoms (`|-`, `:`) interleaved with argument holes;
//! `Mixop` is the form without arguments.
//! EL uses only atoms, which stay in `common::notation`.

mod error;
mod mixfix;
pub mod mixop;

pub use error::ArityMismatch;
pub use mixfix::{AtomPhrase, Mixfix};
pub use mixop::Mixop;
