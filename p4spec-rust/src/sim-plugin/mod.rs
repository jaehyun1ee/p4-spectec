//! Selects an architecture and assembles the native simulator

use self::{
    arch::Architecture,
    ebpf::Ebpf,
    io::Tx,
    psa::Psa,
    runner::{Error, Run},
    v1model::V1Model,
};
use crate::{
    interp::{al::Config, shared::error::Error as InterpError, sl::Config as SlConfig},
    lang::{
        al::ast::Spec,
        common::source::Phrase,
        data::value::{ValueArena, external::Encoding},
    },
    runner::{self as host, BuiltinInterface, Interpreter, Runner},
    stf::ast::Statement,
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
    fn arena(&self) -> &ValueArena;

    fn init_pipe(&mut self, includes: &[PathBuf], path: &Path) -> Result<Run, Error>;

    fn run_stf_stmt(
        &mut self,
        run: &mut Run,
        stmt: &Phrase<Statement>,
    ) -> Result<Option<Tx>, Error>;

    fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
    ) -> Result<Run, Error>;
}

impl<Interp, Arch> SimulatorRunner for Runner<Interp, BuiltinInterface, Arch>
where
    Interp: Interpreter<BuiltinInterface, Arch, Error = InterpError> + 'static,
    Arch: Architecture + 'static,
{
    fn arena(&self) -> &ValueArena {
        Runner::arena(self)
    }

    fn init_pipe(&mut self, includes: &[PathBuf], path: &Path) -> Result<Run, Error> {
        runner::init_pipe(self, includes, path)
    }

    fn run_stf_stmt(
        &mut self,
        run: &mut Run,
        stmt: &Phrase<Statement>,
    ) -> Result<Option<Tx>, Error> {
        runner::run_stf_stmt(self, run, stmt)
    }

    fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
    ) -> Result<Run, Error> {
        runner::run_stf_test(self, includes, path_p4, path_stf)
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

    pub fn arena(&self) -> &ValueArena {
        self.runner.arena()
    }

    /// Starts a fresh program, resetting the arena and all host components
    pub fn init_pipe(&mut self, includes: &[PathBuf], path: &Path) -> Result<Run, Error> {
        self.runner.init_pipe(includes, path)
    }

    pub fn run_stf_stmt(
        &mut self,
        run: &mut Run,
        stmt: &Phrase<Statement>,
    ) -> Result<Option<Tx>, Error> {
        self.runner.run_stf_stmt(run, stmt)
    }

    pub fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
    ) -> Result<Run, Error> {
        self.runner.run_stf_test(includes, path_p4, path_stf)
    }
}

// == Construction

pub fn build(spec: Spec, arch: &str, config: Config) -> Result<Simulator, BuildError> {
    build_with_encoding(spec, arch, config, Encoding::default())
}

pub fn build_with_encoding(
    spec: Spec,
    arch: &str,
    config: Config,
    encoding: Encoding,
) -> Result<Simulator, BuildError> {
    match arch {
        "ebpf" => Ok(Simulator::new(host::build_al(spec, config, Ebpf::new(encoding))?)),
        "psa" => Ok(Simulator::new(host::build_al(spec, config, Psa::new(encoding))?)),
        "v1model" => Ok(Simulator::new(host::build_al(spec, config, V1Model::new(encoding))?)),
        _ => Err(BuildError::UnsupportedArchitecture(arch.to_owned())),
    }
}

pub fn build_sl(spec_al: Spec, arch: &str, config: SlConfig) -> Result<Simulator, BuildError> {
    build_sl_with_encoding(spec_al, arch, config, Encoding::default())
}

pub fn build_sl_with_encoding(
    spec_al: Spec,
    arch: &str,
    config: SlConfig,
    encoding: Encoding,
) -> Result<Simulator, BuildError> {
    match arch {
        "ebpf" => Ok(Simulator::new(host::build_sl(spec_al, config, Ebpf::new(encoding))?)),
        "psa" => Ok(Simulator::new(host::build_sl(spec_al, config, Psa::new(encoding))?)),
        "v1model" => Ok(Simulator::new(host::build_sl(spec_al, config, V1Model::new(encoding))?)),
        _ => Err(BuildError::UnsupportedArchitecture(arch.to_owned())),
    }
}
