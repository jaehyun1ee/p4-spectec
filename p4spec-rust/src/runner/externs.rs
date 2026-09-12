//! Host extern contract for a composed specification runner.
//!
//! Each call receives the operation-local runner context and may reenter the
//! interpreter through it. The result carries its own side-effect flag, so the
//! extern implementation keeps all architecture-specific state bookkeeping.
//! Failures describe the host operation; the calling interpreter owns the
//! source location.

use thiserror::Error;

use crate::{lang::data::value::Value, lang::il::ast::Typ};

use super::{Interface, Interpreter, RunnerContext};

// == Extern errors

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExternError {
    #[error("extern is not configured")]
    NotConfigured,
    #[error(transparent)]
    Value(#[from] crate::lang::data::value::ValueError),
    #[error("{0}")]
    Failure(String),
}

// == Extern contract

pub trait Extern: Sized {
    fn eval_rel<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        values: &[Value],
    ) -> Result<(Vec<Value>, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;

    fn eval_func<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;

    fn clear(&mut self);
}

// == Null implementation

pub struct NullExtern;

impl Extern for NullExtern {
    fn eval_rel<Interp, Iface>(
        &self,
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _name: &str,
        _values: &[Value],
    ) -> Result<(Vec<Value>, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let error = ExternError::NotConfigured;
        Err(error.into())
    }

    fn eval_func<Interp, Iface>(
        &self,
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _name: &str,
        _targs: &[Typ],
        _values: &[Value],
    ) -> Result<(Value, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let error = ExternError::NotConfigured;
        Err(error.into())
    }

    fn clear(&mut self) {}
}
