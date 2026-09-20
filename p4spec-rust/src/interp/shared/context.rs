//! Read and write context interfaces for shared evaluation
//!
//! `ReadContext` looks values up by frame slot and types and functions by id;
//! `WriteContext` adds bindings;
//! `IterContext` batches values across iterated variables for `eval::iter`.
//! AL and SL each implement them on their own context type.

use crate::interp::shared::prepare::ast;
use crate::lang::data::var::{SlotIdx, VarSlot};
use crate::{
    interp::shared::{backtrack::Backtrack, error::Error},
    lang::data::value::{Value, ValueArena},
    runtime::typdef::TypeDef,
};
use std::rc::Rc;

// = Context access

/// Read access to value, type, and function bindings.
pub trait ReadContext {
    // == Values

    /// The value bound at `slot`, if any.
    fn find_value(&self, slot: SlotIdx) -> Option<&Value>;
    /// The slot of `var` under one more iteration `iter`.
    fn find_iter_var(&self, var: &VarSlot, iter: ast::Iter) -> VarSlot;

    // == Types

    /// A type definition by id, if any.
    fn find_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    fn find_typdef_local_opt(&self, id: &ast::Id) -> Option<&TypeDef>;
    /// The parameters and body of a defined type, or an undefined-type error.
    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error>;

    // == Functions

    /// The function definition type this context stores.
    type Func;

    /// A function definition by id.
    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error>;
    /// The type of a function by id.
    fn find_func_typ(&self, id: &ast::Id) -> Result<ast::FuncTyp, Error>;
}

/// Write access to value and function bindings.
pub trait WriteContext: ReadContext + Clone {
    // == Values

    /// Binds a value to a slot.
    fn add_value(&mut self, slot: SlotIdx, value: Value);
    /// Drops every value binding.
    fn clear_value_bindings(&mut self);

    // == Functions

    /// Binds a function definition to an id.
    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error>;
}

/// Context operations used by shared iteration evaluation.
pub trait IterContext: WriteContext {
    // == Input values

    /// The list values bound to `vars`, all of equal length.
    fn find_list_values_by_var<'arena>(
        &self,
        arena: &'arena ValueArena,
        vars: &[ast::Var],
    ) -> Result<Vec<&'arena [Value]>, Error>;

    /// The option values bound to `vars`: all present, or all absent.
    fn find_opt_values_by_var(
        &self,
        arena: &ValueArena,
        vars: &[ast::Var],
    ) -> Result<Option<Vec<Value>>, Error>;

    // == Output bindings

    /// Appends the values bound to `vars` to their columns.
    fn collect_values_by_var(
        &self,
        vars: &[ast::Var],
        values_by_var: &mut [Vec<Value>],
    ) -> Backtrack<()>;

    /// Binds each variable to the list of its column.
    fn bind_list_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        values_by_var: Vec<Vec<Value>>,
    ) -> Backtrack<()>;

    /// Binds each variable to the option built from its column.
    fn bind_opt_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        values_by_var: Vec<Vec<Value>>,
    ) -> Backtrack<()>;
}
