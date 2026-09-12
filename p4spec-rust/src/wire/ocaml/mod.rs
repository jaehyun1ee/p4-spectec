//! Versioned OCaml wire codecs

pub mod lang;
mod reader;

pub use crate::lang::data::serialize::{DecodeError, atom, mixfix, source};
use crate::lang::data::serialize::{
    array, boolean, field, integer, object, on_codec_stack, string, variant,
};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EncodeError {}
