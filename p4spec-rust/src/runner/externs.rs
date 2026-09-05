//! Host extern contract for a composed specification runner.
//!
//! Each call receives the operation-local runner context and may reenter the
//! interpreter through it. The result carries its own side-effect flag, so the
//! extern implementation keeps all architecture-specific state bookkeeping.
//! Failures describe the host operation; the calling interpreter owns the
//! source location.

use std::rc::Rc;

use thiserror::Error;

use crate::{lang::data::value::Value, lang::il::ast::Typ};

use super::{Interface, Interpreter, RunnerContext};

// == Extern errors

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExternError {
    #[error("extern is not configured")]
    NotConfigured,
    #[error("{0}")]
    Failure(String),
}

// == Extern contract

pub trait Extern: Sized {
    fn eval_rel<S, I>(
        &self,
        context: &mut RunnerContext<'_, S, I, Self>,
        name: &str,
        values: &[Rc<Value>],
    ) -> Result<(Vec<Rc<Value>>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>;

    fn eval_func<S, I>(
        &self,
        context: &mut RunnerContext<'_, S, I, Self>,
        name: &str,
        targs: &[Typ],
        values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>;

    fn clear(&mut self);
}

// == Null implementation

pub struct NullExtern;

impl Extern for NullExtern {
    fn eval_rel<S, I>(
        &self,
        _context: &mut RunnerContext<'_, S, I, Self>,
        _name: &str,
        _values: &[Rc<Value>],
    ) -> Result<(Vec<Rc<Value>>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        let error = ExternError::NotConfigured;
        Err(error.into())
    }

    fn eval_func<S, I>(
        &self,
        _context: &mut RunnerContext<'_, S, I, Self>,
        _name: &str,
        _targs: &[Typ],
        _values: &[Rc<Value>],
    ) -> Result<(Rc<Value>, bool), S::Error>
    where
        I: Interface,
        S: Interpreter<I, Self>,
    {
        let error = ExternError::NotConfigured;
        Err(error.into())
    }

    fn clear(&mut self) {}
}
