//! Read and write context interfaces for shared evaluation

use crate::{
    interp::shared::{backtrack::Backtrack, error::Error},
    lang::{
        common::Variable,
        data::value::{Value, ValueArena},
        il::ast,
    },
    runtime::typdef::TypeDef,
};
use std::rc::Rc;

// = Context access

/// Read access to value, type, and function bindings
pub trait ReadContext {
    // == Values

    fn find_value(&self, var: &Variable) -> Result<&Value, Error>;

    // == Types

    fn find_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_local_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error>;

    // == Functions

    type Func;

    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error>;
    fn find_func_typ(&self, id: &ast::Id) -> Result<ast::FuncTyp, Error>;
}

/// Write access to value and function bindings
pub trait WriteContext: ReadContext + Clone {
    // == Values

    fn add_value(&mut self, var: Variable, value: Value);
    fn clear_value_bindings(&mut self);

    // == Functions

    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error>;
}

/// Context operations used by shared iteration evaluation
pub trait IterContext: WriteContext {
    fn opt_values(
        &self,
        arena: &ValueArena,
        vars: &[ast::Var],
    ) -> Result<Option<Vec<Value>>, Error>;
    fn list_values<'arena>(
        &self,
        arena: &'arena ValueArena,
        vars: &[ast::Var],
    ) -> Result<Vec<&'arena [Value]>, Error>;
    fn collect_bindings(&self, vars: &[ast::Var], values_bind: &mut [Vec<Value>]) -> Backtrack<()>;
    fn bind_iter(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        iter: ast::Iter,
        values_bind: Vec<Vec<Value>>,
    ) -> Backtrack<()>;
}
