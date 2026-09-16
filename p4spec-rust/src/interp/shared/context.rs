//! Static bindings needed by the shared expression and assignment operations

use crate::{
    interp::shared::{backtrack::Backtrack, error::Error},
    lang::{
        common::{Variable, source::Span},
        data::value::Value,
        il::ast,
    },
    runner::{Extern, Interface, Interpreter, RunnerContext},
    runtime::{envs::interp::TDEnv, ops::typ::Theta},
};
use std::rc::Rc;

pub trait ValueContext: Clone {
    fn find_value(&self, var: &Variable) -> Result<&Value, Error>;
    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error>;
    fn find_func_typ(&self, id: &ast::Id) -> Result<crate::lang::il::ast::FuncTyp, Error>;
    fn tdenv(&self) -> TDEnv;
    fn theta_local(&self) -> Theta;
}

pub trait AssignContext: ValueContext {
    type Func;
    fn add_value(&mut self, var: Variable, value: Value);
    fn wipe(&self) -> Self;
    fn lookup_func(&self, id: &ast::Id) -> Result<Rc<Self::Func>, Error>;
    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error>;
}

pub(crate) trait EvalContext<Iface: Interface, Exn: Extern>: ValueContext {
    type Interp: Interpreter<Iface, Exn, Error = Error>;
    fn trace_exp(&self, _exp: &ast::Exp, result: Backtrack<Value>) -> Backtrack<Value> {
        result
    }
    fn trace_arg(&self, _arg: &ast::Arg, result: Backtrack<Value>) -> Backtrack<Value> {
        result
    }
    fn invoke_func(
        &self,
        runner: &mut RunnerContext<'_, Self::Interp, Iface, Exn>,
        id: &ast::Id,
        targs: &[ast::Typ],
        values: &[Value],
    ) -> Backtrack<Value>;
    fn map_list(
        &self,
        runner: &mut RunnerContext<'_, Self::Interp, Iface, Exn>,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(&mut RunnerContext<'_, Self::Interp, Iface, Exn>, &Self) -> Backtrack<Value>,
    ) -> Backtrack<Vec<Value>>;
    fn map_opt(
        &self,
        runner: &mut RunnerContext<'_, Self::Interp, Iface, Exn>,
        span: &Span,
        vars: &[ast::Var],
        eval: impl FnMut(&mut RunnerContext<'_, Self::Interp, Iface, Exn>, &Self) -> Backtrack<Value>,
    ) -> Backtrack<Option<Value>>;
}
