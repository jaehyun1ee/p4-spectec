//! Selects an architecture and assembles the native simulator

use self::{arch::Architecture, ebpf::Ebpf, io::Tx, psa::Psa, runner::Error, v1model::V1Model};
use crate::{
    interp::shared::error::Error as InterpError,
    lang::data::value::external::Encoding,
    runner::{self as host, BuiltinInterface, Interpreter, Runner},
};
use std::path::{Path, PathBuf};

pub mod arch;
pub mod core;
pub mod dummy;
pub mod ebpf;
mod externs;
pub mod hash;
pub mod io;
pub mod psa;
pub mod runner;
pub mod spec;
pub mod state;
pub mod table;
pub mod v1model;

// == Build errors

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("architecture {0} is not supported")]
    UnsupportedArchitecture(String),
    #[error(transparent)]
    Runner(#[from] host::BuildError),
}

// == Simulator

trait SimulatorRunner {
    fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
        on_match: &mut dyn FnMut(&Tx),
    ) -> Result<(), Error>;
}

impl<Interp, Arch> SimulatorRunner for Runner<Interp, BuiltinInterface, Arch>
where
    Interp: Interpreter<BuiltinInterface, Arch, Error = InterpError> + 'static,
    Arch: Architecture + 'static,
{
    fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
        on_match: &mut dyn FnMut(&Tx),
    ) -> Result<(), Error> {
        runner::run_stf_test(self, includes, path_p4, path_stf, on_match).map(drop)
    }
}

pub struct Simulator {
    runner: Box<dyn SimulatorRunner>,
}

impl Simulator {
    fn new<Interp, Arch>(runner: Runner<Interp, BuiltinInterface, Arch>) -> Self
    where
        Interp: Interpreter<BuiltinInterface, Arch, Error = InterpError> + 'static,
        Arch: Architecture + 'static,
    {
        Self { runner: Box::new(runner) }
    }

    pub fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
        mut on_match: impl FnMut(&Tx),
    ) -> Result<(), Error> {
        self.runner
            .run_stf_test(includes, path_p4, path_stf, &mut on_match)
    }
}

// == Construction

pub fn build(
    spec: host::Spec,
    arch: &str,
    config: host::Config,
    encoding: Encoding,
) -> Result<Simulator, BuildError> {
    match arch {
        "ebpf" => build_for_arch(spec, config, Ebpf::new(encoding)),
        "psa" => build_for_arch(spec, config, Psa::new(encoding)),
        "v1model" => build_for_arch(spec, config, V1Model::new(encoding)),
        _ => Err(BuildError::UnsupportedArchitecture(arch.to_owned())),
    }
}

fn build_for_arch<Arch: Architecture + 'static>(
    spec: host::Spec,
    config: host::Config,
    arch: Arch,
) -> Result<Simulator, BuildError> {
    match spec {
        host::Spec::Al(spec) => Ok(Simulator::new(host::build_al(spec, config, arch)?)),
        host::Spec::Sl(spec) => Ok(Simulator::new(host::build_sl(spec, config, arch)?)),
    }
}
