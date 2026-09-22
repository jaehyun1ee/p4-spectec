//! Loaded PL definitions and per-call execution bindings
//!
//! `Global::load` prepares callables once for slot execution.
//! A `Context` borrows global definitions and owns local bindings.
//! Cloning preserves the local scope through copy-on-write value frames;
//! `localize` starts a fresh scope over the same global definitions.

use std::rc::Rc;

use crate::{
    interp::shared::{
        backtrack::{Backtrack, ok, unwrap_from_result},
        context::{IterContext, ReadContext, WriteContext},
        error::{ContextErrorKind, EntityKind, Error, ErrorKind},
        prepare::ast as exec,
    },
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make},
            var::{SlotIdx, VarSlot},
        },
        pl::ast as source,
        traits::print::Print,
    },
    runtime::{
        envs::interp::{
            pl::{FEnv, REnv, TDEnv, ast_prepared as ast},
            shared::{callable::Callable, frame::Frame},
        },
        typdef::TypeDef,
    },
};

/// Identifies the scope supplying a function definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Loaded from the specification.
    Global,
    /// Bound by a function argument in the current call.
    Local,
}

/// Stores type definitions and prepared relation and function callables.
#[derive(Debug)]
pub struct Global {
    tdenv: TDEnv,
    renv: REnv,
    fenv: FEnv,
}

impl Global {
    /// Loads type definitions and prepares each callable for slot execution.
    pub fn load(spec: source::Spec) -> Result<Self, Error> {
        let mut loaded = Self { tdenv: TDEnv::new(), renv: REnv::new(), fenv: FEnv::new() };
        // Move source definitions into the execution environments
        for def in spec {
            match def.node.node {
                source::DefKind::Typ(typdef) => {
                    // Types keep their definition body
                    let (id, typdef) = match typdef {
                        source::TypDef::Extern(typdef) => (typdef.id, TypeDef::Extern),
                        source::TypDef::Defined(typdef) => {
                            let source::DefinedTyp { id, tparams, def_typ } = *typdef;
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
                source::DefKind::Var(_) => {}
                source::DefKind::Rel(rel) => {
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
                source::DefKind::MetaFunc(func) => {
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

/// Holds type parameters, function arguments, and values of one call.
#[derive(Clone, Debug, Default)]
struct Local {
    /// Type parameters bound to their type arguments.
    tdenv: TDEnv,
    /// Function arguments bound to their definitions.
    fenv: FEnv,
    /// Value slots of the current callable.
    frame: Frame,
}

/// Combines global definitions with local bindings of one call.
#[derive(Clone, Debug)]
pub struct Context<'global> {
    global: &'global Global,
    local: Local,
}

impl<'global> Context<'global> {
    // == Constructors

    /// Creates a context with no local bindings.
    pub fn new(global: &'global Global) -> Self {
        Self { global, local: Local::default() }
    }

    /// Starts a fresh local scope over the same globals.
    pub fn localize(&self) -> Self {
        Self::new(self.global)
    }

    /// Starts a fresh local scope with the callee's frame layout.
    pub fn localize_with_layout(
        &self,
        layout: &Rc<crate::runtime::envs::interp::shared::frame::FrameLayout>,
    ) -> Self {
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

    pub fn find_typdef(&self, id: &ast::Id) -> Result<&TypeDef, Error> {
        self.find_typdef_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Type, id.node.clone(), id.span.clone()))
    }

    // - Relations

    /// Finds a relation in the global definitions.
    pub fn find_rel(&self, id: &ast::Id) -> Result<&'global Callable<ast::RelDef>, Error> {
        self.global
            .renv
            .get(id)
            .ok_or_else(|| Error::undefined(EntityKind::Relation, id.node.clone(), id.span.clone()))
    }

    // - Functions

    /// Finds a function, with local arguments shadowing global definitions.
    pub fn find_func_opt(&self, id: &ast::Id) -> Option<(Scope, &Rc<Callable<ast::MetaFuncDef>>)> {
        self.local
            .fenv
            .get(id)
            .map(|func| (Scope::Local, func))
            .or_else(|| self.global.fenv.get(id).map(|func| (Scope::Global, func)))
    }

    pub fn find_func_with_scope(
        &self,
        id: &ast::Id,
    ) -> Result<(Scope, &Rc<Callable<ast::MetaFuncDef>>), Error> {
        self.find_func_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Function, id.node.clone(), id.span.clone()))
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
    fn find_iter_var(&self, var: &VarSlot, iter: exec::Iter) -> VarSlot {
        self.local.frame.layout().find_iter_var(var, iter)
    }
    // - Types

    fn find_typdef_local_opt(&self, id: &exec::Id) -> Option<&TypeDef> {
        self.local.tdenv.get(id)
    }
    fn find_typdef_opt(&self, id: &exec::Id) -> Option<&TypeDef> {
        self.local
            .tdenv
            .get(id)
            .or_else(|| self.global.tdenv.get(id))
    }
    fn find_defined_typdef(
        &self,
        id: &exec::Id,
    ) -> Result<(&[exec::TParam], &exec::DefTyp), Error> {
        match self.find_typdef(id)? {
            TypeDef::Defined(tparams, def_typ) => Ok((tparams, def_typ)),
            _ => Err(Error::undefined(EntityKind::DefinedType, id.node.clone(), id.span.clone())),
        }
    }
    // - Functions

    fn find_func(&self, id: &exec::Id) -> Result<&Rc<Self::Func>, Error> {
        self.find_func_with_scope(id).map(|(_, func)| func)
    }
    fn find_func_typ(&self, id: &exec::Id) -> Result<crate::lang::il::ast::FuncTyp, Error> {
        fn param_typ(param: &ast::Param) -> ast::Typ {
            match &param.node {
                ast::ParamKind::Exp(typ, _) => typ.clone(),
                ast::ParamKind::Def(_, tparams, params, typ) => typ::make::func(
                    tparams.clone(),
                    params.iter().map(param_typ).collect(),
                    typ.clone(),
                ),
            }
        }
        let func = self.find_func(id)?;
        let (tparams, params, typ) = match &func.def {
            ast::MetaFuncDef::Extern(def) => (&def.tparams[..], &def.params[..], &def.typ),
            ast::MetaFuncDef::Builtin(def) => (&def.tparams[..], &def.params[..], &def.typ),
            ast::MetaFuncDef::Table(def) => (&[][..], &def.params[..], &def.typ),
            ast::MetaFuncDef::Defined(def) => (&def.tparams[..], &def.params[..], &def.typ),
        };
        Ok(crate::lang::data::typ::FuncTyp {
            tparams: tparams.to_vec(),
            typs_params: params.iter().map(param_typ).collect(),
            typ_ret: Box::new(typ.clone()),
        })
    }
}

// = Write access

impl WriteContext for Context<'_> {
    // == Adders

    // - Types

    fn add_typdef_local(&mut self, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        // A type parameter may shadow a global definition
        if self.local.tdenv.contains_key(&id) {
            return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
        }
        self.local.tdenv.insert(id, typdef);
        Ok(())
    }

    // - Values

    fn add_value(&mut self, slot: SlotIdx, value: Value) {
        self.local.frame.set(slot, value);
    }
    // == Clearing

    fn clear_value_bindings(&mut self) {
        // Keep the layout, drop the values
        self.local.frame = self.local.frame.wipe();
    }
    // == Adders

    // - Functions

    fn add_func(&mut self, id: exec::Id, func: Rc<Self::Func>) -> Result<(), Error> {
        if self.find_func_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Function, id.node, id.span));
        }
        self.local.fenv.insert(id, func);
        Ok(())
    }
}

// = Iteration access

impl IterContext for Context<'_> {
    // == Finders

    // - Values

    fn find_list_values_by_var<'a>(
        &self,
        arena: &'a ValueArena,
        vars: &[exec::Var],
    ) -> Result<Vec<&'a [Value]>, Error> {
        let mut rows = Vec::with_capacity(vars.len());
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
            rows.push(
                get::list(arena, value)
                    .map_err(|error| Error::from(error).at_if_missing(&var.var.id.span))?,
            );
        }
        // All lists must have the same length
        if let Some(first) = rows.first() {
            for row in &rows {
                if row.len() != first.len() {
                    return Err(Error::new(
                        ErrorKind::Context(ContextErrorKind::IterationLengthMismatch {
                            expected: first.len(),
                            actual: row.len(),
                        }),
                        Span::default(),
                    ));
                }
            }
        }
        Ok(rows)
    }

    fn find_opt_values_by_var(
        &self,
        arena: &ValueArena,
        vars: &[exec::Var],
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
            values.push(
                get::opt(arena, value)
                    .map_err(|error| Error::from(error).at_if_missing(&var.var.id.span))?,
            );
        }
        // All present, all absent, or a mismatch
        if values.iter().all(Option::is_some) {
            Ok(Some(values.into_iter().flatten().collect()))
        } else if values.iter().all(Option::is_none) {
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
        vars: &[exec::Var],
        columns: &mut [Vec<Value>],
    ) -> Backtrack<()> {
        // Append this row's value of each variable
        for (var, column) in vars.iter().zip(columns) {
            column.push(*unwrap_from_result!(
                self.find_value(var.slot).ok_or_else(|| Error::undefined(
                    EntityKind::Value,
                    Print::to_string(&var.var),
                    var.var.id.span.clone()
                )),
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
        vars: &[exec::Var],
        columns: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(columns) {
            let typ = typ::make::iterate(var.var.typ.clone(), &var.var.iters);
            // Each variable becomes a list one iteration outward
            let value = unwrap_from_result!(
                make::list(arena, typ.node.into(), values, Span::default()),
                &Span::default()
            );
            self.add_value(var.slot, value);
        }
        ok!(())
    }

    fn bind_opt_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[exec::Var],
        columns: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(columns) {
            let typ = typ::make::iterate(var.var.typ.clone(), &var.var.iters);
            // Each variable becomes an option one iteration outward
            let value = unwrap_from_result!(
                make::opt(arena, typ.node.into(), values.into_iter().next(), Span::default()),
                &Span::default()
            );
            self.add_value(var.slot, value);
        }
        ok!(())
    }
}
