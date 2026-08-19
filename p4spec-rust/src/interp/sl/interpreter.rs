use std::rc::Rc;

use crate::{
    domain::source::{Region, Spanned},
    interface::{Extern, Interface},
    interp::common::InterpError,
    lang::{il::ast::Typ, sl::ast::Def},
    runtime::{
        dynamic::caches::{CallCache, CallKey},
        dynamic_sl::func::Function,
        value::{ValueKind, ValueRef},
    },
};

use super::context::{Context, Cursor};

const CALL_CACHE_SIZE: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Options {
    pub cache: bool,
    pub deterministic: bool,
    pub guard: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cache: false,
            deterministic: false,
            guard: true,
        }
    }
}

pub struct Interpreter<I, E> {
    options: Options,
    context: Context,
    interface: I,
    externs: E,
    function_cache: CallCache<ValueRef>,
}

impl<I, E> Interpreter<I, E>
where
    I: Interface,
    E: Extern,
{
    pub fn new(
        spec: &[Def],
        options: Options,
        interface: I,
        externs: E,
    ) -> Result<Self, InterpError> {
        Ok(Self {
            options,
            context: Context::from_spec(options.deterministic, spec)?,
            interface,
            externs,
            function_cache: CallCache::new(CALL_CACHE_SIZE),
        })
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    fn clear_call_caches(&mut self) {
        self.function_cache.clear();
    }

    pub fn eval_func(
        &mut self,
        name: &str,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        self.clear_call_caches();
        self.invoke_func(
            &Spanned::new(name.to_owned(), Region::none()),
            type_args,
            values_input,
        )
    }

    fn invoke_func(
        &mut self,
        id: &crate::lang::il::ast::Id,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        let (cursor, function) = self.context.find_function(id)?;
        let function = function.clone();
        let cacheable = self.options.cache
            && cursor != Cursor::Local
            && !matches!(function, Function::Extern(..))
            && !values_input
                .iter()
                .any(|value| matches!(value.kind, ValueKind::FuncV(_)));
        let key: CallKey = (id.node.clone(), values_input.to_vec());
        if cacheable && let Some(value) = self.function_cache.find(&key) {
            return Ok(Rc::clone(value));
        }

        let interface_before = self.interface.checkpoint();
        let extern_before = self.externs.checkpoint();
        let value = self.invoke_func_body(id, &function, type_args, values_input)?;
        if cacheable
            && !self
                .interface
                .side_effected(interface_before, self.interface.checkpoint())
            && !self
                .externs
                .side_effected(extern_before, self.externs.checkpoint())
        {
            self.function_cache.insert(key, Rc::clone(&value));
        }
        Ok(value)
    }

    fn invoke_func_body(
        &mut self,
        id: &crate::lang::il::ast::Id,
        function: &Function,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        match function {
            Function::Extern(..) => self
                .externs
                .eval_func(&id.node, type_args, values_input)
                .map_err(|error| InterpError::new(error.span, error.message)),
            Function::Builtin(..) => self
                .interface
                .call_builtin(&mut |_| {}, id, type_args, values_input)
                .map_err(|error| InterpError::new(error.span, error.message)),
            Function::Table(..) => Err(InterpError::new(
                id.span.clone(),
                "table function execution is not implemented",
            )),
            Function::Defined(..) => Err(InterpError::new(
                id.span.clone(),
                "defined function execution is not implemented",
            )),
        }
    }
}
