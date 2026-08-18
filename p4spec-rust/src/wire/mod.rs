//! Transport formats used to cross the OCaml/Rust process boundary

mod envelope;
pub mod ocaml;

pub use envelope::{Envelope, SL_SCHEMA, VALUE_SCHEMA, WireError};
