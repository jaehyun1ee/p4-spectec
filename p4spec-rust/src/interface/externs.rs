use thiserror::Error;

use crate::{domain::source::Region, lang::il::ast::Typ, runtime::value::ValueRef};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message} at {span}")]
pub struct ExternError {
    pub span: Region,
    pub message: String,
}

impl ExternError {
    pub fn new(span: Region, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}

// Interface for the interaction between SpecTec and external code

pub trait SpecCall {
    fn eval_func(
        &mut self,
        name: &str,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError>;

    fn eval_rel(&mut self, name: &str, values: &[ValueRef]) -> Result<Vec<ValueRef>, ExternError>;
}

pub trait Extern {
    // Extern relation and meta-function evaluation

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
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError>;

    // State management

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
        Err(ExternError::new(Region::none(), "extern is not configured"))
    }

    fn eval_func(
        &mut self,
        _spec: &mut dyn SpecCall,
        _name: &str,
        _type_args: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        Err(ExternError::new(Region::none(), "extern is not configured"))
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}
