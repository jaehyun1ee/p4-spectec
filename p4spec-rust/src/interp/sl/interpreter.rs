use std::rc::Rc;

use crate::{
    domain::source::{Region, Spanned},
    interface::{Extern, ExternError, Interface, NullExtern, SpecCall},
    interp::common::InterpError,
    lang::{
        il::ast::{DefTypKind, TParam, Typ},
        sl::ast::{Block, Def, ElseBlock, Param, ParamKind},
    },
    runtime::{
        dynamic::caches::{CallCache, CallKey, CallKeyRef},
        dynamic_sl::envs::TypeDefEnv,
        dynamic_sl::func::Function,
        dynamic_sl::rel::Relation,
        r#type::{subst, typdef::TypeDef},
        value::{ValueKind, ValueRef, get, r#match as value_match},
    },
};

use super::{
    assignment,
    context::{Context, Cursor},
    expression::{self, Calls},
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
    sub_cache: value_match::SubCache,
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
            sub_cache: value_match::SubCache::new(),
        })
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    fn clear_call_caches(&mut self) {
        self.function_cache.clear();
        self.relation_cache.clear();
        self.sub_cache.clear();
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
            sub_cache: &mut self.sub_cache,
        };
        dispatcher.check_func_inputs(
            &self.context,
            &Spanned::new(name.to_owned(), Region::none()),
            type_args,
            values_input,
        )?;
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
            sub_cache: &mut self.sub_cache,
        };
        dispatcher.check_rel_inputs(
            &self.context,
            &Spanned::new(name.to_owned(), Region::none()),
            values_input,
        )?;
        dispatcher.invoke_rel(
            &mut self.context,
            &Spanned::new(name.to_owned(), Region::none()),
            values_input,
        )
    }

    pub fn eval_program(
        &mut self,
        relation: &str,
        program: &ValueRef,
    ) -> Result<Vec<ValueRef>, InterpError> {
        self.eval_rel(relation, std::slice::from_ref(program))
    }
}

struct Dispatcher<'a, I, E> {
    options: Options,
    interface: &'a mut I,
    externs: &'a mut E,
    function_cache: &'a mut CallCache<ValueRef>,
    relation_cache: &'a mut CallCache<Vec<ValueRef>>,
    sub_cache: &'a mut value_match::SubCache,
}

struct SpecDispatcher<'a, I> {
    options: Options,
    context: &'a mut Context,
    interface: &'a mut I,
    function_cache: &'a mut CallCache<ValueRef>,
    relation_cache: &'a mut CallCache<Vec<ValueRef>>,
    sub_cache: &'a mut value_match::SubCache,
}

impl<I> SpecCall for SpecDispatcher<'_, I>
where
    I: Interface,
{
    fn eval_func(
        &mut self,
        name: &str,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        let id = Spanned::new(name.to_owned(), Region::none());
        let mut externs = NullExtern;
        let mut dispatcher = Dispatcher {
            options: self.options,
            interface: &mut *self.interface,
            externs: &mut externs,
            function_cache: &mut *self.function_cache,
            relation_cache: &mut *self.relation_cache,
            sub_cache: &mut *self.sub_cache,
        };
        dispatcher
            .check_func_inputs(self.context, &id, type_args, values_input)
            .and_then(|()| dispatcher.invoke_func(self.context, &id, type_args, values_input))
            .map_err(|error| ExternError::new(error.span, error.message))
    }
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
    // Checkers

    fn find_func_signature(context: &Context, name: &str) -> Option<value_match::FuncSignature> {
        let id = Spanned::new(name.to_owned(), Region::none());
        context.find_function(&id).ok().map(|(_cursor, function)| {
            let signature = function.get_signature();
            value_match::FuncSignature {
                type_params: signature.type_params,
                param_types: signature.param_types,
                return_type: signature.return_type,
            }
        })
    }

    fn values_match(
        context: &Context,
        types: &[Typ],
        values: &[ValueRef],
    ) -> Result<bool, InterpError> {
        value_match::subs(
            &context.type_defs(),
            &|name| Self::find_func_signature(context, name),
            types,
            values,
        )
        .map_err(|error| InterpError::new(Region::none(), error.to_string()))
    }

    fn check_rel_inputs(
        &self,
        context: &Context,
        id: &crate::lang::il::ast::Id,
        values_input: &[ValueRef],
    ) -> Result<(), InterpError> {
        if !self.options.guard {
            return Ok(());
        }
        let relation = context.find_relation(id)?;
        let (not_type, inputs) = relation.get_signature();
        let types = not_type.node.args();
        let types_input = inputs
            .iter()
            .map(|index| {
                usize::try_from(*index)
                    .ok()
                    .and_then(|index| types.get(index).copied())
                    .cloned()
                    .ok_or_else(|| {
                        InterpError::new(id.span.clone(), "relation input hint is out of bounds")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if Self::values_match(context, &types_input, values_input)? {
            Ok(())
        } else {
            Err(InterpError::new(
                id.span.clone(),
                format!(
                    "relation input of {} does not match the expected type",
                    id.node
                ),
            ))
        }
    }

    fn check_rel_outputs(
        &self,
        context: &Context,
        id: &crate::lang::il::ast::Id,
        relation: &Relation,
        values_output: &[ValueRef],
    ) -> Result<(), InterpError> {
        if !self.options.guard {
            return Ok(());
        }
        let (not_type, inputs) = relation.get_signature();
        let types_output = not_type
            .node
            .args()
            .into_iter()
            .enumerate()
            .filter(|(index, _typ)| !inputs.contains(&(*index as i64)))
            .map(|(_index, typ)| typ.clone())
            .collect::<Vec<_>>();
        if Self::values_match(context, &types_output, values_output)? {
            Ok(())
        } else {
            Err(InterpError::new(
                id.span.clone(),
                format!(
                    "relation output of {} does not match the expected type",
                    id.node
                ),
            ))
        }
    }

    fn check_func_inputs(
        &self,
        context: &Context,
        id: &crate::lang::il::ast::Id,
        type_args: &[Typ],
        values_input: &[ValueRef],
    ) -> Result<(), InterpError> {
        if !self.options.guard {
            return Ok(());
        }
        let function = context.find_function(id)?.1;
        let signature = function.get_signature();
        if signature.type_params.len() != type_args.len() {
            return Err(InterpError::new(
                id.span.clone(),
                format!("arity mismatch in type arguments of {}", id.node),
            ));
        }
        let substitution = signature
            .type_params
            .iter()
            .zip(type_args)
            .map(|(type_param, type_arg)| (type_param.node.clone(), type_arg.clone()))
            .collect();
        let param_types = subst::subst_types(&substitution, &signature.param_types)
            .map_err(|error| InterpError::new(id.span.clone(), error.to_string()))?;
        if Self::values_match(context, &param_types, values_input)? {
            Ok(())
        } else {
            Err(InterpError::new(
                id.span.clone(),
                format!(
                    "function argument of {} does not match the parameter type",
                    id.node
                ),
            ))
        }
    }

    fn check_func_output(
        &mut self,
        context: &Context,
        id: &crate::lang::il::ast::Id,
        type_params: &[TParam],
        return_type: &Typ,
        type_args: &[Typ],
        value_output: &ValueRef,
    ) -> Result<(), InterpError> {
        if !self.options.guard {
            return Ok(());
        }
        if type_params.len() != type_args.len() {
            return Err(InterpError::new(
                id.span.clone(),
                format!("arity mismatch in type arguments of {}", id.node),
            ));
        }
        let substitution = type_params
            .iter()
            .zip(type_args)
            .map(|(type_param, type_arg)| (type_param.node.clone(), type_arg.clone()))
            .collect();
        let return_type = subst::subst_type(&substitution, return_type)
            .map_err(|error| InterpError::new(id.span.clone(), error.to_string()))?;
        let matches = value_match::sub(
            self.sub_cache,
            &context.type_defs(),
            &|name| Self::find_func_signature(context, name),
            &return_type,
            value_output,
        )
        .map_err(|error| InterpError::new(id.span.clone(), error.to_string()))?;
        if matches {
            Ok(())
        } else {
            Err(InterpError::new(
                id.span.clone(),
                format!(
                    "return value of function {} does not match the expected type",
                    id.node
                ),
            ))
        }
    }

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
            let function = Rc::clone(function);
            let cacheable = self.options.cache
                && cursor != Cursor::Local
                && !matches!(function.as_ref(), Function::Extern(..))
                && !values_input
                    .iter()
                    .any(|value| matches!(value.kind, ValueKind::FuncV(_)));
            let key = CallKeyRef::new(&id.node, &values_input);
            let result = if cacheable && let Some(value) = self.function_cache.find(&key) {
                FunctionResult::Return(Rc::clone(value))
            } else {
                let interface_before = self.interface.checkpoint();
                let extern_before = self.externs.checkpoint();
                let result = self.invoke_func_body(
                    context,
                    &id,
                    function.as_ref(),
                    &type_args,
                    &values_input,
                )?;
                if cacheable
                    && let FunctionResult::Return(value) = &result
                    && !self
                        .interface
                        .side_effected(interface_before, self.interface.checkpoint())
                    && !self
                        .externs
                        .side_effected(extern_before, self.externs.checkpoint())
                {
                    self.function_cache.insert(
                        CallKey::new(id.node.clone(), values_input.clone()),
                        Rc::clone(value),
                    );
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
            Function::Extern(type_params, _params, return_type) => {
                let value = {
                    let mut spec = SpecDispatcher {
                        options: self.options,
                        context,
                        interface: &mut *self.interface,
                        function_cache: &mut *self.function_cache,
                        relation_cache: &mut *self.relation_cache,
                        sub_cache: &mut *self.sub_cache,
                    };
                    self.externs
                        .eval_func(&mut spec, &id.node, type_args, values_input)
                        .map_err(|error| InterpError::new(error.span, error.message))?
                };
                self.check_func_output(context, id, type_params, return_type, type_args, &value)?;
                Ok(FunctionResult::Return(value))
            }
            Function::Builtin(type_params, _params, return_type) => {
                let value = self
                    .interface
                    .call_builtin(&mut |_| {}, id, type_args, values_input)
                    .map_err(|error| InterpError::new(error.span, error.message))?;
                self.check_func_output(context, id, type_params, return_type, type_args, &value)?;
                Ok(FunctionResult::Return(value))
            }
            Function::Table(params, _return_type, rows) => {
                if !type_args.is_empty() {
                    return Err(InterpError::new(
                        id.span.clone(),
                        "arity mismatch in type arguments",
                    ));
                }
                let param_functions =
                    Self::prepare_param_functions(context, id, params, values_input)?;
                context
                    .with_function_frame(
                        id.clone(),
                        values_input.to_vec(),
                        TypeDefEnv::new(),
                        |context| {
                            Self::assign_params(context, params, values_input, &param_functions)?;
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

    fn prepare_param_functions(
        context: &Context,
        id: &crate::lang::il::ast::Id,
        params: &[Param],
        values: &[ValueRef],
    ) -> Result<Vec<Option<Rc<Function>>>, InterpError> {
        if params.len() != values.len() {
            return Err(InterpError::new(
                id.span.clone(),
                "arity mismatch in function arguments",
            ));
        }
        params
            .iter()
            .zip(values)
            .map(|(param, value)| match &param.node {
                ParamKind::ExpP(..) => Ok(None),
                ParamKind::DefP(..) => {
                    let function_id = get::func(value)
                        .map_err(|error| InterpError::new(param.span.clone(), error.to_string()))?;
                    context
                        .find_function(function_id)
                        .map(|(_cursor, function)| Some(Rc::clone(function)))
                }
            })
            .collect()
    }

    fn assign_params(
        context: &mut Context,
        params: &[Param],
        values: &[ValueRef],
        param_functions: &[Option<Rc<Function>>],
    ) -> Result<(), InterpError> {
        debug_assert_eq!(params.len(), values.len());
        debug_assert_eq!(params.len(), param_functions.len());
        for ((param, value), function) in params.iter().zip(values).zip(param_functions) {
            match &param.node {
                ParamKind::ExpP(_typ, exp) => {
                    assignment::assign(context, exp, Rc::clone(value))?;
                }
                ParamKind::DefP(id, _type_params, _params, _return_type) => {
                    context.bind_function(
                        id.clone(),
                        function
                            .clone()
                            .expect("definition parameters were resolved in the caller"),
                    )?;
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
        let param_functions = Self::prepare_param_functions(context, id, params, values_input)?;
        context.with_function_frame(id.clone(), values_input.to_vec(), type_defs, |context| {
            Self::assign_params(context, params, values_input, &param_functions)?;
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
            let relation = Rc::clone(context.find_relation(&id)?);
            let cacheable = self.options.cache && !matches!(relation.as_ref(), Relation::Extern(_));
            let key = CallKeyRef::new(&id.node, &values_input);
            let result = if cacheable && let Some(values) = self.relation_cache.find(&key) {
                RelationResult::Result(values.clone())
            } else {
                let interface_before = self.interface.checkpoint();
                let extern_before = self.externs.checkpoint();
                let result =
                    self.invoke_rel_body(context, &id, relation.as_ref(), &values_input)?;
                if cacheable
                    && let RelationResult::Result(values) = &result
                    && !self
                        .interface
                        .side_effected(interface_before, self.interface.checkpoint())
                    && !self
                        .externs
                        .side_effected(extern_before, self.externs.checkpoint())
                {
                    self.relation_cache.insert(
                        CallKey::new(id.node.clone(), values_input.clone()),
                        values.clone(),
                    );
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
            Relation::Extern(_) => {
                let values = {
                    let mut spec = SpecDispatcher {
                        options: self.options,
                        context,
                        interface: &mut *self.interface,
                        function_cache: &mut *self.function_cache,
                        relation_cache: &mut *self.relation_cache,
                        sub_cache: &mut *self.sub_cache,
                    };
                    self.externs
                        .eval_rel(&mut spec, &id.node, values_input)
                        .map_err(|error| InterpError::new(error.span, error.message))?
                };
                self.check_rel_outputs(context, id, relation, &values)?;
                Ok(RelationResult::Result(values))
            }
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
    fn value_is_subtype(
        &mut self,
        context: &Context,
        typ: &Typ,
        value: &ValueRef,
    ) -> Result<bool, InterpError> {
        expression::value_is_subtype(self.sub_cache, context, typ, value)
    }

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
