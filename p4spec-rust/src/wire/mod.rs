//! Formats used at external compatibility boundaries

mod envelope;

pub mod ocaml;

pub use envelope::{
    AL_SCHEMA, EL_SCHEMA, Envelope, IL_SCHEMA, PL_SCHEMA, SL_SCHEMA, VALUE_SCHEMA, WireError,
};
