use std::rc::Rc;

use crate::{
    domain::source::{Region, Spanned},
    interface::{Extern, Interface},
    interp::common::InterpError,
    lang::{
        il::ast::{DefTypKind, TParam, Typ},
        sl::ast::{Block, Def, ElseBlock, Param, ParamKind},
    },
    runtime::{
        dynamic::caches::{CallCache, CallKey},
        dynamic_sl::envs::TypeDefEnv,
        dynamic_sl::func::Function,
        r#type::typdef::TypeDef,
        value::{ValueKind, ValueRef, get},
    },
};

use super::{
    assignment,
    context::{Context, Cursor},
    expression::FunctionCalls,
    instruction::{self, Flow},
};

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
        let mut dispatcher = FunctionDispatcher {
            options: self.options,
            interface: &mut self.interface,
            externs: &mut self.externs,
            function_cache: &mut self.function_cache,
        };
        dispatcher.invoke_func(
            &mut self.context,
            &Spanned::new(name.to_owned(), Region::none()),
            type_args,
            values_input,
        )
    }
}

struct FunctionDispatcher<'a, I, E> {
    options: Options,
    interface: &'a mut I,
    externs: &'a mut E,
    function_cache: &'a mut CallCache<ValueRef>,
}

impl<I, E> FunctionDispatcher<'_, I, E>
where
    I: Interface,
    E: Extern,
{
    fn invoke_func(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        let (cursor, function) = context.find_function(id)?;
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
        let value = self.invoke_func_body(context, id, &function, type_args, values_input)?;
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
        context: &mut Context,
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
            Function::Defined(type_params, params, _return_type, block, else_block) => self
                .invoke_defined_func(
                    context,
                    id,
                    type_params,
                    params,
                    block,
                    else_block.as_ref(),
                    type_args,
                    values_input,
                ),
        }
    }

    fn local_type_defs(
        id: &crate::lang::il::ast::Id,
        type_params: &[TParam],
        type_args: &[Typ],
    ) -> Result<TypeDefEnv, InterpError> {
        if type_params.len() != type_args.len() {
            return Err(InterpError::new(
                id.span.clone(),
                "arity mismatch in type arguments",
            ));
        }
        Ok(type_params
            .iter()
            .zip(type_args)
            .map(|(type_param, type_arg)| {
                (
                    type_param.node.clone(),
                    TypeDef::Defined(
                        Vec::new(),
                        Box::new(Spanned::new(
                            DefTypKind::PlainT(type_arg.clone()),
                            type_arg.span.clone(),
                        )),
                    ),
                )
            })
            .collect())
    }

    fn assign_params(
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        params: &[Param],
        values: &[ValueRef],
    ) -> Result<(), InterpError> {
        if params.len() != values.len() {
            return Err(InterpError::new(
                id.span.clone(),
                "arity mismatch in function arguments",
            ));
        }
        for (param, value) in params.iter().zip(values) {
            match &param.node {
                ParamKind::ExpP(_typ, exp) => {
                    assignment::assign(context, exp, Rc::clone(value))?;
                }
                ParamKind::DefP(id, _type_params, _params, _return_type) => {
                    let function_id = get::func(value)
                        .map_err(|error| InterpError::new(param.span.clone(), error.to_string()))?;
                    let function = context.find_function(function_id)?.1.clone();
                    context.bind_function(id.clone(), function)?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn invoke_defined_func(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        type_params: &[TParam],
        params: &[Param],
        block: &Block,
        else_block: Option<&ElseBlock>,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        let type_defs = Self::local_type_defs(id, type_params, type_args)?;
        context.with_function_frame(id.clone(), values_input.to_vec(), type_defs, |context| {
            Self::assign_params(context, id, params, values_input)?;
            let flow = instruction::eval_block(context, self, block)?;
            let flow = match (flow, else_block) {
                (Flow::Continue, Some(else_block)) => {
                    instruction::eval_block(context, self, else_block)?
                }
                (flow, _) => flow,
            };
            instruction::return_value(flow, &id.span)
        })
    }
}

impl<I, E> FunctionCalls for FunctionDispatcher<'_, I, E>
where
    I: Interface,
    E: Extern,
{
    fn invoke_func(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        Self::invoke_func(self, context, id, type_args, values)
    }
}
