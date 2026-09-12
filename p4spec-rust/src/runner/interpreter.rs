//! Stage-specific evaluation contract used by a composed runner.
//!
//! An interpreter owns its configuration and cache for one language stage.
//! Evaluation receives the assembled runner context, including the interpreter,
//! so extern calls can reenter without a second mutable interpreter borrow.

use crate::{
    lang::{data::value::Value, il::ast::Typ},
    runner::{ExternError, InterfaceError},
};

use super::{Extern, Interface, RunnerContext};

// == Interpreter contract

pub trait Interpreter<Iface, Exn>: Sized
where
    Iface: Interface,
    Exn: Extern,
{
    type Spec;
    type Error: From<InterfaceError> + From<ExternError>;

    /// Clears cached results without invalidating arena values
    fn clear(&mut self);

    /// Resets program-owned execution state while retaining configuration
    fn reset(&mut self);

    /// Evaluates an already parsed program through the selected entry
    fn eval_program(
        ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        program: Value,
    ) -> Result<Vec<Value>, Self::Error>;

    fn eval_rel(
        ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        values: &[Value],
    ) -> Result<Vec<Value>, Self::Error>;

    fn eval_func(
        ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        targs: &[Typ],
        values: &[Value],
    ) -> Result<Value, Self::Error>;
}
