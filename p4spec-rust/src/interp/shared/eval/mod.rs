//! Shared assignment, expression, argument, and path evaluation

pub(crate) mod arg;
pub mod assign;
pub(crate) mod expr;
pub mod iter;
pub(crate) mod ops;
pub(crate) mod path;

use super::{backtrack::Backtrack, context::IterContext, error::Error};
use crate::{
    lang::{data::value::Value, il::ast},
    runner::{Extern, Interface, Interpreter, RunnerContext},
};

// = Invocation

/// AL/SL-specific function and relation invocation
pub(crate) trait Invoker<Iface, Ext>: Interpreter<Iface, Ext, Error = Error>
where
    Iface: Interface,
    Ext: Extern,
{
    type Context<'global>: IterContext;

    fn invoke_func<'global>(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        ctx: &Self::Context<'global>,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value>;

    fn invoke_rel<'global>(
        runner_ctx: &mut RunnerContext<'_, Self, Iface, Ext>,
        ctx: &Self::Context<'global>,
        id: &ast::Id,
        values: &[Value],
    ) -> Backtrack<Vec<Value>>;
}
