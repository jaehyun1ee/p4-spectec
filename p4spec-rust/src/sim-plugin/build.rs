//! Selects an architecture and assembles the native AL simulator

use super::{
    ebpf::Ebpf,
    io::Tx,
    psa::Psa,
    runner::{self, Error, Run},
    v1model::V1Model,
};
use crate::{
    interface::p4::unparse::P4Unparser,
    interp::al::{AlInterp, Config, context::Global, error::Error as InterpError},
    lang::{
        al::ast::Spec,
        common::source::Phrase,
        data::value::{ValueArena, external::Encoding},
    },
    runner::{BuiltinInterface, Runner},
    stf::ast::Statement,
};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("architecture {0} is not supported")]
    UnsupportedArchitecture(String),
    #[error(transparent)]
    Spec(#[from] InterpError),
}

pub enum Simulator {
    Ebpf(Box<Runner<AlInterp, BuiltinInterface, Ebpf>>),
    Psa(Box<Runner<AlInterp, BuiltinInterface, Psa>>),
    V1Model(Box<Runner<AlInterp, BuiltinInterface, V1Model>>),
}

pub fn build(spec: Spec, arch: &str, config: Config) -> Result<Simulator, BuildError> {
    build_with_encoding(spec, arch, config, Encoding::default())
}

pub fn build_with_encoding(
    spec: Spec,
    arch: &str,
    config: Config,
    encoding: Encoding,
) -> Result<Simulator, BuildError> {
    if !matches!(arch, "ebpf" | "psa" | "v1model") {
        return Err(BuildError::UnsupportedArchitecture(arch.to_owned()));
    }
    let unparser = P4Unparser::from_al_spec(&spec);
    let global = Global::load(spec)?;
    let interp = AlInterp::new(config);
    let interface = BuiltinInterface::new(unparser);
    Ok(match arch {
        "ebpf" => Simulator::Ebpf(Box::new(Runner::new(
            global,
            interp,
            interface,
            Ebpf::new(encoding),
        ))),
        "psa" => Simulator::Psa(Box::new(Runner::new(
            global,
            interp,
            interface,
            Psa::new(encoding),
        ))),
        "v1model" => Simulator::V1Model(Box::new(Runner::new(
            global,
            interp,
            interface,
            V1Model::new(encoding),
        ))),
        _ => unreachable!("architecture checked before specification loading"),
    })
}

macro_rules! dispatch {
    ($sim:expr, $runner:ident => $body:expr) => {
        match $sim {
            Simulator::Ebpf($runner) => $body,
            Simulator::Psa($runner) => $body,
            Simulator::V1Model($runner) => $body,
        }
    };
}

impl Simulator {
    pub fn arena(&self) -> &ValueArena {
        dispatch!(self, runner => runner.arena())
    }

    /// Starts a fresh program, resetting the arena and all host components
    pub fn init_pipe(&mut self, includes: &[PathBuf], path: &Path) -> Result<Run, Error> {
        dispatch!(self, runner => runner::init_pipe(runner, includes, path))
    }

    pub fn run_stf_stmt(
        &mut self,
        run: &mut Run,
        stmt: &Phrase<Statement>,
    ) -> Result<Option<Tx>, Error> {
        dispatch!(self, runner => runner::run_stf_stmt(runner, run, stmt))
    }

    pub fn run_stf_test(
        &mut self,
        includes: &[PathBuf],
        path_p4: &Path,
        path_stf: &Path,
    ) -> Result<Run, Error> {
        dispatch!(self, runner => runner::run_stf_test(runner, includes, path_p4, path_stf))
    }
}
