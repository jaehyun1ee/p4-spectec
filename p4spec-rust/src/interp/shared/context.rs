//! Environment lookup and binding interfaces for shared evaluation

use crate::{
    interp::shared::error::Error,
    lang::{common::Variable, data::value::Value, il::ast},
    runtime::typdef::TypeDef,
};
use std::rc::Rc;

// = Environment

/// Read access to value, type, and function bindings
pub trait Environment {
    fn find_value(&self, var: &Variable) -> Result<&Value, Error>;
    fn find_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_local_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error>;
    fn find_func_typ(&self, id: &ast::Id) -> Result<ast::FuncTyp, Error>;
}

// = Value bindings

pub trait Bindings: Environment + Clone {
    fn add_value(&mut self, var: Variable, value: Value);
    fn with_empty_values(&self) -> Self;
}

// = Function bindings

pub trait FuncBindings {
    type Func;

    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error>;
    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error>;
}
