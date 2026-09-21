//! Environments grouped by their runtime consumer
//!
//! `elab` and `algo` map names to static shapes;
//! `interp` maps them to prepared callables and frames.

pub mod algo;
pub mod elab;
pub mod interp;
