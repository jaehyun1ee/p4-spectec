//! Language models and codecs for P4 `SpecTec`
//!
//! `frontend` parses the specification into EL;
//! `pass` elaborates it to IL, converts to AL, and structures into SL;
//! `interp` and `runner` execute AL or SL against a P4 program,
//! with `interface` builtins and `sim_plugin` architectures;
//! `lang`, `runtime`, `stf`, and `util` are the shared data and codecs.

pub mod diagnostic;
pub mod frontend;
pub mod interface;
pub mod interp;
pub mod lang;
pub mod pass;
pub mod runner;
pub mod runtime;
#[path = "sim-plugin/mod.rs"]
pub mod sim_plugin;
pub mod stf;
pub mod util;
