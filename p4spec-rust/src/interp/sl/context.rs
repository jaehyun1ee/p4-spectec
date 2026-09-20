//! Loaded definitions and persistent local execution bindings
//!
//! `Global::load` owns global definitions.
//! A `Context` borrows them and owns persistent local bindings.
//! Cloning preserves the local scope;
//! `localize` starts a fresh scope while retaining the same global definitions.

use crate::runtime::envs::interp::sl::ast_prepared as ast;
use std::rc::Rc;

use crate::interp::shared::context::{IterContext, ReadContext, WriteContext};
use crate::interp::shared::error::ContextErrorKind;
use crate::lang::data::var::{SlotIdx, VarSlot};
use crate::runtime::envs::interp::shared::{
    callable::Callable,
    frame::{Frame, FrameLayout},
};

use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make},
        },
        traits::print::Print,
    },
    runtime::{
        envs::interp::sl::{FEnv, REnv, TDEnv},
        typdef::TypeDef,
    },
};

use crate::interp::shared::{
    backtrack::{Backtrack, ok, unwrap_from_result},
    error::{EntityKind, Error, ErrorKind},
};

/// Where a function definition was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Loaded from the specification.
    Global,
    /// Bound by a function argument in the current call.
    Local,
}

/// Type, relation, and function definitions of the loaded specification.
#[derive(Debug)]
pub struct Global {
    tdenv: TDEnv,
    renv: REnv,
    fenv: FEnv,
}

impl Global {
    /// Loads a specification and prepares its callables for slot execution.
    pub fn load(spec: crate::lang::sl::ast::Spec) -> Result<Self, Error> {
        let mut loaded = Self { tdenv: TDEnv::new(), renv: REnv::new(), fenv: FEnv::new() };
        for def in spec {
            match def.node {
                crate::lang::sl::ast::DefKind::Typ(typdef) => {
                    // Types keep their definition body
                    let (id, typdef) = match typdef {
                        ast::TypDef::Extern(typdef) => (typdef.id, TypeDef::Extern),
                        ast::TypDef::Defined(typdef) => {
                            let ast::DefinedTyp { id, tparams, def_typ, .. } = *typdef;
                            (id, TypeDef::Defined(tparams, Box::new(def_typ)))
                        }
                    };
                    // Ids are unique per namespace
                    if loaded.tdenv.contains_key(&id) {
                        return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
                    }
                    loaded.tdenv.insert(id, typdef);
                }
                // Meta-variables carry no runtime state
                crate::lang::sl::ast::DefKind::Var(_) => {}
                crate::lang::sl::ast::DefKind::Rel(rel) => {
                    // Relations are prepared into callables with a frame layout
                    let rel = Callable::prepare(rel);
                    let id = match &rel.def {
                        ast::RelDef::Extern(rel) => &rel.id,
                        ast::RelDef::Defined(rel) => &rel.id,
                    };
                    if loaded.renv.contains_key(id) {
                        return Err(Error::duplicate(
                            EntityKind::Relation,
                            id.node.clone(),
                            id.span.clone(),
                        ));
                    }
                    loaded.renv.insert(id.clone(), rel);
                }
                crate::lang::sl::ast::DefKind::MetaFunc(func) => {
                    // So are functions, shared through `Rc` for function values
                    let func = Callable::prepare(func);
                    let id = match &func.def {
                        ast::MetaFuncDef::Extern(func) => &func.id,
                        ast::MetaFuncDef::Builtin(func) => &func.id,
                        ast::MetaFuncDef::Table(func) => &func.id,
                        ast::MetaFuncDef::Defined(func) => &func.id,
                    };
                    if loaded.fenv.contains_key(id) {
                        return Err(Error::duplicate(
                            EntityKind::Function,
                            id.node.clone(),
                            id.span.clone(),
                        ));
                    }
                    loaded.fenv.insert(id.clone(), Rc::new(func));
                }
            }
        }
        Ok(loaded)
    }
}

/// Bindings of one call: type parameters, function arguments, and the frame.
#[derive(Clone, Debug, Default)]
struct Local {
    /// Type parameters bound to their type arguments.
    tdenv: TDEnv,
    /// Function arguments bound to their definitions.
    fenv: FEnv,
    /// Value slots of the current callable.
    frame: Frame,
}

/// Global definitions plus the local bindings of one call.
#[derive(Clone, Debug)]
pub struct Context<'global> {
    global: &'global Global,
    local: Local,
}

impl<'global> Context<'global> {
    // == Constructors

    /// A context with no local bindings.
    pub fn new(global: &'global Global) -> Self {
        Self { global, local: Local::default() }
    }

    /// A fresh local scope over the same globals.
    pub fn localize(&self) -> Self {
        Self::new(self.global)
    }

    /// A fresh local scope whose frame follows the callee's layout.
    pub fn localize_with_layout(&self, layout: &Rc<FrameLayout>) -> Self {
        Self {
            global: self.global,
            local: Local {
                tdenv: TDEnv::new(),
                fenv: FEnv::new(),
                frame: Frame::new(Rc::clone(layout)),
            },
        }
    }

    // == Finders

    // - Types

    pub fn find_typdef<'a>(&'a self, id: &ast::Id) -> Result<&'a TypeDef, Error> {
        self.find_typdef_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Type, id.node.clone(), id.span.clone()))
    }

    // - Relations

    /// A relation by id; relations are global only.
    pub fn find_rel_opt(&self, id: &ast::Id) -> Option<&'global Callable<ast::RelDef>> {
        self.global.renv.get(id)
    }

    pub fn find_rel(&self, id: &ast::Id) -> Result<&'global Callable<ast::RelDef>, Error> {
        self.find_rel_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Relation, id.node.clone(), id.span.clone()))
    }

    // - Functions

    /// A function by id, local bindings shadowing global definitions.
    pub fn find_func_opt<'a>(
        &'a self,
        id: &ast::Id,
    ) -> Option<(Scope, &'a Rc<Callable<ast::MetaFuncDef>>)> {
        if let Some(func) = self.local.fenv.get(id) {
            Some((Scope::Local, func))
        } else {
            self.global.fenv.get(id).map(|func| (Scope::Global, func))
        }
    }

    pub fn find_func_with_scope<'a>(
        &'a self,
        id: &ast::Id,
    ) -> Result<(Scope, &'a Rc<Callable<ast::MetaFuncDef>>), Error> {
        self.find_func_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Function, id.node.clone(), id.span.clone()))
    }

    // == Adders

    // - Types

    /// Binds a type parameter locally; the id must be new in the local scope.
    pub(crate) fn bind_tparam(&mut self, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        if self.local.tdenv.contains_key(&id) {
            return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
        }
        self.local.tdenv.insert(id, typdef);
        Ok(())
    }

    /// Binds a type locally; the id must be new in both scopes.
    pub fn add_typdef(&mut self, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        if self.find_typdef_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
        }
        self.local.tdenv.insert(id, typdef);
        Ok(())
    }
}

// = Read access

impl ReadContext for Context<'_> {
    type Func = Callable<ast::MetaFuncDef>;

    // == Finders

    // - Values

    fn find_value(&self, slot: SlotIdx) -> Option<&Value> {
        self.local.frame.get(slot)
    }

    fn find_iter_var(&self, var: &VarSlot, iter: ast::Iter) -> VarSlot {
        self.local.frame.layout().find_iter_var(var, iter)
    }

    // - Types

    fn find_typdef_local_opt(&self, id: &ast::Id) -> Option<&TypeDef> {
        self.local.tdenv.get(id)
    }

    fn find_typdef_opt<'a>(&'a self, id: &ast::Id) -> Option<&'a TypeDef> {
        // Local type parameters shadow global types
        self.find_typdef_local_opt(id)
            .or_else(|| self.global.tdenv.get(id))
    }

    fn find_defined_typdef<'a>(
        &'a self,
        id: &ast::Id,
    ) -> Result<(&'a [ast::TParam], &'a ast::DefTyp), Error> {
        match self.find_typdef(id)? {
            TypeDef::Defined(tparams, def_typ) => Ok((tparams, def_typ)),
            _ => Err(Error::undefined(EntityKind::DefinedType, id.node.clone(), id.span.clone())),
        }
    }

    // - Functions

    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error> {
        self.find_func_with_scope(id).map(|(_, func)| func)
    }

    fn find_func_typ(&self, id: &ast::Id) -> Result<crate::lang::il::ast::FuncTyp, Error> {
        use crate::lang::data::typ::{FuncTyp, make};

        fn param_typ(param: &ast::Param) -> ast::Typ {
            match &param.node {
                ast::ParamKind::Exp(typ, _) => typ.clone(),
                // A function parameter has a function type
                ast::ParamKind::Def(_, tparams, params, typ) => {
                    make::func(tparams.clone(), params.iter().map(param_typ).collect(), typ.clone())
                }
            }
        }
        // Read the signature parts off whichever definition kind
        let func = self.find_func(id)?;
        let (tparams, params, typ): (&[ast::TParam], &[ast::Param], &ast::Typ) = match &func.def {
            ast::MetaFuncDef::Extern(func) => (&func.tparams, &func.params, &func.typ),
            ast::MetaFuncDef::Builtin(func) => (&func.tparams, &func.params, &func.typ),
            // Table functions have no type parameters
            ast::MetaFuncDef::Table(func) => (&[], &func.params, &func.typ),
            ast::MetaFuncDef::Defined(func) => (&func.tparams, &func.params, &func.typ),
        };
        Ok(FuncTyp {
            tparams: tparams.to_vec(),
            typs_params: params.iter().map(param_typ).collect(),
            typ_ret: Box::new(typ.clone()),
        })
    }
}

// = Write access

impl WriteContext for Context<'_> {
    // == Adders

    // - Values

    fn add_value(&mut self, slot: SlotIdx, value: Value) {
        self.local.frame.set(slot, value);
    }

    // - Functions

    fn add_func(&mut self, id: ast::Id, func: Rc<Callable<ast::MetaFuncDef>>) -> Result<(), Error> {
        if self.find_func_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Function, id.node, id.span));
        }
        self.local.fenv.insert(id, func);
        Ok(())
    }

    // == Clearing

    fn clear_value_bindings(&mut self) {
        // Keep the layout, drop the values
        self.local.frame = self.local.frame.wipe();
    }
}

// = Iteration access

impl IterContext for Context<'_> {
    // == Finders

    // - Values

    fn find_list_values_by_var<'a>(
        &self,
        arena: &'a ValueArena,
        vars: &[ast::Var],
    ) -> Result<Vec<&'a [Value]>, Error> {
        let mut values_by_var = Vec::with_capacity(vars.len());
        for var in vars {
            // Every variable must be bound
            let value = self.find_value(var.slot).ok_or_else(|| {
                Error::undefined(
                    EntityKind::Value,
                    Print::to_string(&var.var),
                    var.var.id.span.clone(),
                )
            })?;
            // Each variable must hold a list
            let values = get::list(arena, value)
                .map_err(|error| Error::from(error).at_if_missing(&var.var.id.span))?;
            values_by_var.push(values);
        }
        // No variables: nothing to iterate
        let Some(values) = values_by_var.first() else {
            return Ok(Vec::new());
        };
        // All lists must have the same length
        let len = values.len();
        for values in &values_by_var {
            if values.len() != len {
                return Err(Error::new(
                    ErrorKind::Context(ContextErrorKind::IterationLengthMismatch {
                        expected: len,
                        actual: values.len(),
                    }),
                    Span::default(),
                ));
            }
        }
        Ok(values_by_var)
    }

    fn find_opt_values_by_var(
        &self,
        arena: &ValueArena,
        vars: &[ast::Var],
    ) -> Result<Option<Vec<Value>>, Error> {
        let mut values = Vec::with_capacity(vars.len());
        for var in vars {
            // Every variable must be bound
            let value = self.find_value(var.slot).ok_or_else(|| {
                Error::undefined(
                    EntityKind::Value,
                    Print::to_string(&var.var),
                    var.var.id.span.clone(),
                )
            })?;
            // Each variable must hold an option
            let value = get::opt(arena, value)
                .map_err(|error| Error::from(error).at_if_missing(&var.var.id.span))?;
            values.push(value);
        }
        // All present, all absent, or a mismatch
        if values.iter().all(|value| value.is_some()) {
            Ok(Some(values.into_iter().flatten().collect()))
        } else if values.iter().all(|value| value.is_none()) {
            Ok(None)
        } else {
            Err(Error::new(
                ErrorKind::Context(ContextErrorKind::OptionalityMismatch),
                Span::default(),
            ))
        }
    }

    // == Collectors

    // - Values

    fn collect_values_by_var(
        &self,
        vars: &[ast::Var],
        values_by_var: &mut [Vec<Value>],
    ) -> Backtrack<()> {
        // Append this row's value of each variable
        for (var, values) in vars.iter().zip(values_by_var) {
            values.push(*unwrap_from_result!(
                self.find_value(var.slot).ok_or_else(|| {
                    Error::undefined(
                        EntityKind::Value,
                        Print::to_string(&var.var),
                        var.var.id.span.clone(),
                    )
                }),
                &var.var.id.span
            ));
        }
        ok!(())
    }

    // == Adders

    // - Values

    fn bind_list_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        values_by_var: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(values_by_var) {
            let typ = typ::make::iterate(var.var.typ.clone(), &var.var.iters);
            // Each variable becomes a list one iteration outward
            let value = make::list(arena, typ.node.into(), values, Span::default());
            let value = unwrap_from_result!(value, &Span::default());
            self.add_value(var.slot, value);
        }
        ok!(())
    }

    fn bind_opt_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        values_by_var: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(values_by_var) {
            let typ = typ::make::iterate(var.var.typ.clone(), &var.var.iters);
            // Each variable becomes an option one iteration outward
            let value =
                make::opt(arena, typ.node.into(), values.into_iter().next(), Span::default());
            let value = unwrap_from_result!(value, &Span::default());
            self.add_value(var.slot, value);
        }
        ok!(())
    }
}
