//! Host extern contract for a composed specification runner.
//!
//! Each call receives the operation-local runner context and may reenter the
//! interpreter through it. The result carries its own side-effect flag, so the
//! extern implementation keeps all architecture-specific state bookkeeping.

use std::rc::Rc;

use thiserror::Error;

use crate::{
    lang::data::value::Value,
    lang::{common::source::Span, il::ast::Typ},
};

use super::{Interface, Interpreter, RunnerContext};

// == Extern errors

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ExternErrorKind {
    #[error("extern is not configured")]
    NotConfigured,
    #[error("{0}")]
    Failure(String),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{kind} at {span}")]
pub struct ExternError {
    pub kind: ExternErrorKind,
    pub span: Span,
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
        let error = not_configured();
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
        let error = not_configured();
        Err(error.into())
    }

    fn clear(&mut self) {}
}

fn not_configured() -> ExternError {
    ExternError {
        kind: ExternErrorKind::NotConfigured,
        span: Span::default(),
    }
}
