//! Language models and codecs for P4 `SpecTec`
//!
//! `parse`, `elab`, `algo`, `structure`, and `annotate` transform source paths
//! into EL, IL, AL, SL, and PL through `frontend` and `pass`;
//! `interp` and `runner` execute AL, SL, or PL against a P4 program,
//! with `interface` builtins and `sim_plugin` architectures;
//! `lang`, `runtime`, `stf`, and `util` are the shared data and codecs.

pub mod frontend;
pub mod interface;
pub mod interp;
pub mod lang;
pub mod pass;
mod pipeline;
pub mod runner;
pub mod runtime;
#[path = "sim-plugin/mod.rs"]
pub mod sim_plugin;
pub mod stf;
pub mod util;

pub use pipeline::{Error, algo, annotate, elab, parse, structure};
