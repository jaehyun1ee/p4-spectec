//! Language models and codecs for P4 `SpecTec`

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
pub mod wire;
