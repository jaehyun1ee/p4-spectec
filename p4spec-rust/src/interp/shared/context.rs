//! Read and write context interfaces for shared evaluation

use crate::interp::shared::prepare::expr as ast;
use crate::lang::data::var::{IdSlot, SlotIdx, VarSlot};
use crate::{
    interp::shared::{backtrack::Backtrack, error::Error},
    lang::data::value::{Value, ValueArena},
    runtime::typdef::TypeDef,
};
use std::rc::Rc;

// = Context access

/// Read access to value, type, and function bindings
pub trait ReadContext {
    // == Values

    fn find_id_value(&self, id: &IdSlot) -> Result<&Value, Error>;
    fn find_value(&self, slot: &VarSlot) -> Result<&Value, Error>;
    fn iter_slot(&self, slot: &VarSlot, iter: ast::Iter) -> VarSlot;

    // == Types

    fn find_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_typdef_local_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error>;

    // == Functions

    type Func;

    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error>;
    fn find_func_typ(&self, id: &ast::Id) -> Result<ast::FuncTyp, Error>;
}

/// Write access to value and function bindings
pub trait WriteContext: ReadContext + Clone {
    // == Values

    fn add_slot(&mut self, slot: SlotIdx, value: Value);
    fn clear_value_bindings(&mut self);

    // == Functions

    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error>;
}

/// Context operations used by shared iteration evaluation
pub trait IterContext: WriteContext {
    // == Input values

    fn find_list_values_by_var<'arena>(
        &self,
        arena: &'arena ValueArena,
        vars: &[ast::Var],
    ) -> Result<Vec<&'arena [Value]>, Error>;

    fn find_opt_values_by_var(
        &self,
        arena: &ValueArena,
        vars: &[ast::Var],
    ) -> Result<Option<Vec<Value>>, Error>;

    // == Output bindings

    fn collect_values_by_var(
        &self,
        vars: &[ast::Var],
        values_by_var: &mut [Vec<Value>],
    ) -> Backtrack<()>;

    fn bind_list_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        values_by_var: Vec<Vec<Value>>,
    ) -> Backtrack<()>;

    fn bind_opt_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        values_by_var: Vec<Vec<Value>>,
    ) -> Backtrack<()>;
}
