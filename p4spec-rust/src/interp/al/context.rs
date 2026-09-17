//! Loaded definitions and persistent local execution bindings
//!
//! `Global::load` owns global definitions. A `Context` borrows them and owns
//! persistent local bindings. Cloning preserves the local scope; `localize`
//! starts a fresh scope while retaining the same global definitions.

use std::rc::Rc;

use crate::interp::shared::error::ContextErrorKind;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::{
            typ,
            value::{Value, ValueArena, get, make},
        },
    },
    runtime::{
        envs::{
            interp::VEnv,
            interp_al::{FEnv, REnv, TDEnv},
        },
        typdef::TypeDef,
    },
};

use crate::interp::shared::{
    backtrack::{Backtrack, backtrack_from_result},
    error::{EntityKind, Error, ErrorKind},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Local,
}

#[derive(Debug)]
pub struct Global {
    tdenv: TDEnv,
    renv: REnv,
    fenv: FEnv,
}

impl Global {
    pub fn load(spec: ast::Spec) -> Result<Self, Error> {
        let mut loaded = Self { tdenv: TDEnv::new(), renv: REnv::new(), fenv: FEnv::new() };
        for def in spec {
            match def.node {
                ast::DefKind::Typ(typdef) => {
                    let (id, typdef) = match typdef {
                        ast::TypDef::Extern(typdef) => (typdef.id, TypeDef::Extern),
                        ast::TypDef::Defined(typdef) => {
                            let ast::DefinedTyp { id, tparams, def_typ, .. } = *typdef;
                            (id, TypeDef::Defined(tparams, Box::new(def_typ)))
                        }
                    };
                    if loaded.tdenv.contains_key(&id) {
                        return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
                    }
                    loaded.tdenv.insert(id, typdef);
                }
                ast::DefKind::Var(_) => {}
                ast::DefKind::Rel(rel) => {
                    let id = match &rel {
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
                ast::DefKind::MetaFunc(func) => {
                    let id = match &func {
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

#[derive(Clone, Debug, Default)]
struct Local {
    tdenv: TDEnv,
    fenv: FEnv,
    venv: VEnv,
}

#[derive(Clone, Debug)]
pub struct Context<'global> {
    global: &'global Global,
    local: Local,
}

impl<'global> Context<'global> {
    // == Constructors

    pub fn new(global: &'global Global) -> Self {
        Self { global, local: Local::default() }
    }

    pub fn localize(&self) -> Self {
        Self::new(self.global)
    }

    pub fn clear_value_bindings(&mut self) {
        self.local.venv = VEnv::new();
    }

    // == Finders

    pub fn find_value_opt(&self, var: &Variable) -> Option<&Value> {
        self.local.venv.get(var)
    }

    pub fn find_value(&self, var: &Variable) -> Result<&Value, Error> {
        self.find_value_opt(var).ok_or_else(|| {
            Error::undefined(EntityKind::Value, var.to_string(), var.id.span.clone())
        })
    }

    pub fn find_local_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef> {
        self.local.tdenv.get(id)
    }

    pub fn find_typdef_opt<'a>(&'a self, id: &ast::Id) -> Option<&'a TypeDef> {
        self.find_local_typdef_opt(id)
            .or_else(|| self.global.tdenv.get(id))
    }

    pub fn find_typdef<'a>(&'a self, id: &ast::Id) -> Result<&'a TypeDef, Error> {
        self.find_typdef_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Type, id.node.clone(), id.span.clone()))
    }

    pub fn find_defined_typdef<'a>(
        &'a self,
        id: &ast::Id,
    ) -> Result<(&'a [ast::TParam], &'a ast::DefTyp), Error> {
        match self.find_typdef(id)? {
            TypeDef::Defined(tparams, def_typ) => Ok((tparams, def_typ)),
            _ => Err(Error::undefined(EntityKind::DefinedType, id.node.clone(), id.span.clone())),
        }
    }

    pub fn find_rel_opt(&self, id: &ast::Id) -> Option<&'global ast::RelDef> {
        self.global.renv.get(id)
    }

    pub fn find_rel(&self, id: &ast::Id) -> Result<&'global ast::RelDef, Error> {
        self.find_rel_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Relation, id.node.clone(), id.span.clone()))
    }

    pub fn find_func_opt<'a>(&'a self, id: &ast::Id) -> Option<(Scope, &'a Rc<ast::MetaFuncDef>)> {
        if let Some(func) = self.local.fenv.get(id) {
            Some((Scope::Local, func))
        } else {
            self.global.fenv.get(id).map(|func| (Scope::Global, func))
        }
    }

    pub fn find_func<'a>(
        &'a self,
        id: &ast::Id,
    ) -> Result<(Scope, &'a Rc<ast::MetaFuncDef>), Error> {
        self.find_func_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Function, id.node.clone(), id.span.clone()))
    }

    pub fn find_func_typ(&self, id: &ast::Id) -> Result<crate::lang::il::ast::FuncTyp, Error> {
        use crate::lang::data::typ::{FuncTyp, make};

        fn param_typ(param: &ast::Param) -> ast::Typ {
            match &param.node {
                ast::ParamKind::Exp(typ) => typ.clone(),
                ast::ParamKind::Def(_, tparams, params, typ) => {
                    make::func(tparams.clone(), params.iter().map(param_typ).collect(), typ.clone())
                }
            }
        }
        let (_, func) = self.find_func(id)?;
        let (tparams, params, typ): (&[ast::TParam], &[ast::Param], &ast::Typ) = match func.as_ref()
        {
            ast::MetaFuncDef::Extern(func) => (&func.tparams, &func.params, &func.typ),
            ast::MetaFuncDef::Builtin(func) => (&func.tparams, &func.params, &func.typ),
            ast::MetaFuncDef::Table(func) => (&[], &func.params, &func.typ),
            ast::MetaFuncDef::Defined(func) => (&func.tparams, &func.params, &func.typ),
        };
        Ok(FuncTyp {
            tparams: tparams.to_vec(),
            typs_params: params.iter().map(param_typ).collect(),
            typ_ret: Box::new(typ.clone()),
        })
    }

    // == Adders

    pub fn add_value(&mut self, var: Variable, value: Value) {
        self.local.venv.insert(var, value);
    }

    pub fn add_typdef(&mut self, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        if self.find_typdef_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
        }
        self.local.tdenv.insert(id, typdef);
        Ok(())
    }

    pub fn add_func(&mut self, id: ast::Id, func: Rc<ast::MetaFuncDef>) -> Result<(), Error> {
        if self.find_func_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Function, id.node, id.span));
        }
        self.local.fenv.insert(id, func);
        Ok(())
    }

    // == Iteration contexts

    // - Iteration inputs

    pub(super) fn opt_values(
        &self,
        arena: &ValueArena,
        vars: &[ast::Var],
    ) -> Result<Option<Vec<Value>>, Error> {
        let mut values = Vec::with_capacity(vars.len());
        for var in vars {
            let mut iters = var.iters.clone();
            iters.push(ast::Iter::Opt);
            let value = self.find_value(&Variable::new(var.id.clone(), iters))?;
            let value = get::opt(arena, value)
                .map_err(|error| Error::from(error).at_if_missing(&var.id.span))?;
            values.push(value);
        }
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

    pub(super) fn list_values<'a>(
        &self,
        arena: &'a ValueArena,
        vars: &[ast::Var],
    ) -> Result<Vec<&'a [Value]>, Error> {
        let mut rows = Vec::with_capacity(vars.len());
        for var in vars {
            let mut iters = var.iters.clone();
            iters.push(ast::Iter::List);
            let value = self.find_value(&Variable::new(var.id.clone(), iters))?;
            let values = get::list(arena, value)
                .map_err(|error| Error::from(error).at_if_missing(&var.id.span))?;
            rows.push(values);
        }
        let Some(row) = rows.first() else {
            return Ok(Vec::new());
        };
        let width = row.len();
        for row in &rows {
            if row.len() != width {
                return Err(Error::new(
                    ErrorKind::Context(ContextErrorKind::IterationLengthMismatch {
                        expected: width,
                        actual: row.len(),
                    }),
                    Span::default(),
                ));
            }
        }
        Ok(rows)
    }

    // - Iteration outputs

    pub(super) fn collect_bindings(
        &self,
        vars: &[ast::Var],
        values_bind: &mut [Vec<Value>],
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(values_bind) {
            let var_bound = Variable::new(var.id.clone(), var.iters.clone());
            values.push(*backtrack_from_result!(self.find_value(&var_bound), &var.id.span));
        }
        Backtrack::Ok(())
    }

    pub(super) fn bind_iter(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        iter: ast::Iter,
        values_bind: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(values_bind) {
            let mut iters = var.iters.clone();
            iters.push(iter);
            let typ = typ::make::iterate(var.typ.clone(), &iters);
            let value = match iter {
                ast::Iter::Opt => {
                    make::opt(arena, typ.node.into(), values.into_iter().next(), Span::default())
                }
                ast::Iter::List => make::list(arena, typ.node.into(), values, Span::default()),
            };
            let value = backtrack_from_result!(value, &Span::default());
            self.add_value(Variable::new(var.id.clone(), iters), value);
        }
        Backtrack::Ok(())
    }
}

// = Shared binding interfaces

impl crate::interp::shared::context::ReadContext for Context<'_> {
    type Func = ast::MetaFuncDef;

    fn find_value(&self, var: &Variable) -> Result<&Value, Error> {
        self.find_value(var)
    }

    fn find_defined_typdef(&self, id: &ast::Id) -> Result<(&[ast::TParam], &ast::DefTyp), Error> {
        self.find_defined_typdef(id)
    }

    fn find_func_typ(&self, id: &ast::Id) -> Result<crate::lang::il::ast::FuncTyp, Error> {
        self.find_func_typ(id)
    }

    fn find_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef> {
        self.find_typdef_opt(id)
    }

    fn find_local_typdef_opt(&self, id: &ast::Id) -> Option<&TypeDef> {
        self.find_local_typdef_opt(id)
    }

    fn find_func(&self, id: &ast::Id) -> Result<&Rc<Self::Func>, Error> {
        self.find_func(id).map(|(_, func)| func)
    }
}

impl crate::interp::shared::context::WriteContext for Context<'_> {
    fn add_value(&mut self, var: Variable, value: Value) {
        self.add_value(var, value)
    }

    fn clear_value_bindings(&mut self) {
        self.clear_value_bindings()
    }

    fn add_func(&mut self, id: ast::Id, func: Rc<Self::Func>) -> Result<(), Error> {
        self.add_func(id, func)
    }
}

impl crate::interp::shared::context::IterContext for Context<'_> {
    fn opt_values(
        &self,
        arena: &ValueArena,
        vars: &[ast::Var],
    ) -> Result<Option<Vec<Value>>, Error> {
        self.opt_values(arena, vars)
    }

    fn list_values<'arena>(
        &self,
        arena: &'arena ValueArena,
        vars: &[ast::Var],
    ) -> Result<Vec<&'arena [Value]>, Error> {
        self.list_values(arena, vars)
    }

    fn collect_bindings(&self, vars: &[ast::Var], values_bind: &mut [Vec<Value>]) -> Backtrack<()> {
        self.collect_bindings(vars, values_bind)
    }

    fn bind_iter(
        &mut self,
        arena: &mut ValueArena,
        vars: &[ast::Var],
        iter: ast::Iter,
        values_bind: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        self.bind_iter(arena, vars, iter, values_bind)
    }
}
