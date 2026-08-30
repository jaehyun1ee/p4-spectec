use thiserror::Error;

use crate::{
    lang::{common::source::Span, il::ast::Typ},
    runtime::value::ValueRef,
};

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

pub trait SpecCall {
    fn eval_func(
        &mut self,
        name: &str,
        targs: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError>;

    fn eval_rel(&mut self, name: &str, values: &[ValueRef]) -> Result<Vec<ValueRef>, ExternError>;
}

pub trait Extern {
    fn eval_rel(
        &mut self,
        spec: &mut dyn SpecCall,
        name: &str,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError>;

    fn eval_func(
        &mut self,
        spec: &mut dyn SpecCall,
        name: &str,
        targs: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError>;

    fn checkpoint(&self) -> u64;

    fn side_effected(&self, before: u64, after: u64) -> bool {
        before != after
    }

    fn clear(&mut self);
}

pub struct NullExtern;

impl Extern for NullExtern {
    fn eval_rel(
        &mut self,
        _spec: &mut dyn SpecCall,
        _name: &str,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        Err(not_configured())
    }

    fn eval_func(
        &mut self,
        _spec: &mut dyn SpecCall,
        _name: &str,
        _targs: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        Err(not_configured())
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

fn not_configured() -> ExternError {
    ExternError {
        kind: ExternErrorKind::NotConfigured,
        span: Span::default(),
    }
}
