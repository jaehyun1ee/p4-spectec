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
        dynamic_sl::rel::Relation,
        r#type::typdef::TypeDef,
        value::{ValueKind, ValueRef, get},
    },
};

use super::{
    assignment,
    context::{Context, Cursor},
    expression::Calls,
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
    relation_cache: CallCache<Vec<ValueRef>>,
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
            relation_cache: CallCache::new(CALL_CACHE_SIZE),
        })
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    fn clear_call_caches(&mut self) {
        self.function_cache.clear();
        self.relation_cache.clear();
    }

    pub fn eval_func(
        &mut self,
        name: &str,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, InterpError> {
        self.clear_call_caches();
        let mut dispatcher = Dispatcher {
            options: self.options,
            interface: &mut self.interface,
            externs: &mut self.externs,
            function_cache: &mut self.function_cache,
            relation_cache: &mut self.relation_cache,
        };
        dispatcher.invoke_func(
            &mut self.context,
            &Spanned::new(name.to_owned(), Region::none()),
            type_args,
            values_input,
        )
    }

    pub fn eval_rel(
        &mut self,
        name: &str,
        values_input: &[ValueRef],
    ) -> Result<Vec<ValueRef>, InterpError> {
        self.clear_call_caches();
        let mut dispatcher = Dispatcher {
            options: self.options,
            interface: &mut self.interface,
            externs: &mut self.externs,
            function_cache: &mut self.function_cache,
            relation_cache: &mut self.relation_cache,
        };
        dispatcher.invoke_rel(
            &mut self.context,
            &Spanned::new(name.to_owned(), Region::none()),
            values_input,
        )
    }
}

struct Dispatcher<'a, I, E> {
    options: Options,
    interface: &'a mut I,
    externs: &'a mut E,
    function_cache: &'a mut CallCache<ValueRef>,
    relation_cache: &'a mut CallCache<Vec<ValueRef>>,
}

enum FunctionResult {
    Return(ValueRef),
    TailCall(crate::lang::il::ast::Id, Vec<Typ>, Vec<ValueRef>),
}

enum RelationResult {
    Result(Vec<ValueRef>),
    TailCall(crate::lang::il::ast::Id, Vec<ValueRef>),
}

impl<I, E> Dispatcher<'_, I, E>
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
        let mut id = id.clone();
        let mut type_args = type_args.to_vec();
        let mut values_input = values_input.to_vec();
        loop {
            let (cursor, function) = context.find_function(&id)?;
            let function = function.clone();
            let cacheable = self.options.cache
                && cursor != Cursor::Local
                && !matches!(function, Function::Extern(..))
                && !values_input
                    .iter()
                    .any(|value| matches!(value.kind, ValueKind::FuncV(_)));
            let key: CallKey = (id.node.clone(), values_input.clone());
            let result = if cacheable && let Some(value) = self.function_cache.find(&key) {
                FunctionResult::Return(Rc::clone(value))
            } else {
                let interface_before = self.interface.checkpoint();
                let extern_before = self.externs.checkpoint();
                let result =
                    self.invoke_func_body(context, &id, &function, &type_args, &values_input)?;
                if cacheable
                    && let FunctionResult::Return(value) = &result
                    && !self
                        .interface
                        .side_effected(interface_before, self.interface.checkpoint())
                    && !self
                        .externs
                        .side_effected(extern_before, self.externs.checkpoint())
                {
                    self.function_cache.insert(key, Rc::clone(value));
                }
                result
            };
            match result {
                FunctionResult::Return(value) => return Ok(value),
                FunctionResult::TailCall(id_tail, type_args_tail, values_tail) => {
                    id = id_tail;
                    type_args = type_args_tail;
                    values_input = values_tail;
                }
            }
        }
    }

    fn invoke_func_body(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        function: &Function,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<FunctionResult, InterpError> {
        match function {
            Function::Extern(..) => self
                .externs
                .eval_func(&id.node, type_args, values_input)
                .map(FunctionResult::Return)
                .map_err(|error| InterpError::new(error.span, error.message)),
            Function::Builtin(..) => self
                .interface
                .call_builtin(&mut |_| {}, id, type_args, values_input)
                .map(FunctionResult::Return)
                .map_err(|error| InterpError::new(error.span, error.message)),
            Function::Table(params, _return_type, rows) => {
                if !type_args.is_empty() {
                    return Err(InterpError::new(
                        id.span.clone(),
                        "arity mismatch in type arguments",
                    ));
                }
                context
                    .with_function_frame(
                        id.clone(),
                        values_input.to_vec(),
                        TypeDefEnv::new(),
                        |context| {
                            Self::assign_params(context, id, params, values_input)?;
                            let flow = instruction::eval_table_rows(context, self, rows)?;
                            instruction::return_value(flow, &id.span)
                        },
                    )
                    .map(FunctionResult::Return)
            }
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
    ) -> Result<FunctionResult, InterpError> {
        let type_defs = Self::local_type_defs(id, type_params, type_args)?;
        context.with_function_frame(id.clone(), values_input.to_vec(), type_defs, |context| {
            Self::assign_params(context, id, params, values_input)?;
            let flow = instruction::eval_block(context, self, block, else_block.is_none())?;
            let flow = match (flow, else_block) {
                (Flow::Continue, Some(else_block)) => {
                    instruction::eval_block(context, self, else_block, true)?
                }
                (flow, _) => flow,
            };
            match flow {
                Flow::TailCallFunc(id, type_args, values) => {
                    Ok(FunctionResult::TailCall(id, type_args, values))
                }
                flow => instruction::return_value(flow, &id.span).map(FunctionResult::Return),
            }
        })
    }

    fn invoke_rel(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        values_input: &[ValueRef],
    ) -> Result<Vec<ValueRef>, InterpError> {
        let mut id = id.clone();
        let mut values_input = values_input.to_vec();
        loop {
            let relation = context.find_relation(&id)?.clone();
            let cacheable = self.options.cache && !matches!(relation, Relation::Extern(_));
            let key: CallKey = (id.node.clone(), values_input.clone());
            let result = if cacheable && let Some(values) = self.relation_cache.find(&key) {
                RelationResult::Result(values.clone())
            } else {
                let interface_before = self.interface.checkpoint();
                let extern_before = self.externs.checkpoint();
                let result = self.invoke_rel_body(context, &id, &relation, &values_input)?;
                if cacheable
                    && let RelationResult::Result(values) = &result
                    && !self
                        .interface
                        .side_effected(interface_before, self.interface.checkpoint())
                    && !self
                        .externs
                        .side_effected(extern_before, self.externs.checkpoint())
                {
                    self.relation_cache.insert(key, values.clone());
                }
                result
            };
            match result {
                RelationResult::Result(values) => return Ok(values),
                RelationResult::TailCall(id_tail, values_tail) => {
                    id = id_tail;
                    values_input = values_tail;
                }
            }
        }
    }

    fn invoke_rel_body(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        relation: &Relation,
        values_input: &[ValueRef],
    ) -> Result<RelationResult, InterpError> {
        match relation {
            Relation::Extern(_) => self
                .externs
                .eval_rel(&id.node, values_input)
                .map(RelationResult::Result)
                .map_err(|error| InterpError::new(error.span, error.message)),
            Relation::Defined(_signature, exps_input, block, else_block) => context
                .with_relation_frame(id.clone(), values_input.to_vec(), |context| {
                    if exps_input.len() != values_input.len() {
                        return Err(InterpError::unmatch(
                            id.span.clone(),
                            "arity mismatch in relation arguments",
                        ));
                    }
                    for (exp, value) in exps_input.iter().zip(values_input) {
                        assignment::assign(context, exp, Rc::clone(value))?;
                    }
                    let flow = instruction::eval_block(context, self, block, else_block.is_none())?;
                    let flow = match (flow, else_block) {
                        (Flow::Continue, Some(else_block)) => {
                            instruction::eval_block(context, self, else_block, true)?
                        }
                        (flow, _) => flow,
                    };
                    match flow {
                        Flow::TailCallRel(id, values) => Ok(RelationResult::TailCall(id, values)),
                        flow => {
                            instruction::result_values(flow, &id.span).map(RelationResult::Result)
                        }
                    }
                }),
        }
    }
}

impl<I, E> Calls for Dispatcher<'_, I, E>
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

    fn invoke_rel(
        &mut self,
        context: &mut Context,
        id: &crate::lang::il::ast::Id,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, InterpError> {
        Self::invoke_rel(self, context, id, values)
    }
}
