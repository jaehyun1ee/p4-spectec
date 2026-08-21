//! Transport formats used to cross the OCaml/Rust process boundary

mod envelope;
pub mod ocaml;
pub mod runtime_value;
pub mod sim_suite;

pub use envelope::{Envelope, SIM_SUITE_SCHEMA, SL_SCHEMA, VALUE_SCHEMA, WireError};
