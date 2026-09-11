//! Loaded definitions and persistent local execution bindings
//!
//! `Global::load` owns global definitions. A `Context` borrows them and owns
//! persistent local bindings. Cloning preserves the local scope; `localize`
//! starts a fresh scope while retaining the same global definitions.

use std::rc::Rc;

use crate::interp::al::error::ContextErrorKind;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::{
            typ,
            value::{Value, ValueArena, get, make},
        },
    },
    runner::{Extern, Interface, RunnerContext},
    runtime::{
        envs::{
            interp::VEnv,
            interp_al::{FEnv, REnv, TDEnv},
        },
        typdef::TypeDef,
    },
};

use super::{
    Al,
    backtrack::{Backtrack, backtrack, backtrack_from_result},
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
        let mut loaded = Self {
            tdenv: TDEnv::new(),
            renv: REnv::new(),
            fenv: FEnv::new(),
        };
        for def in spec {
            match def.node {
                ast::DefKind::Typ(typdef) => {
                    let (id, typdef) = match typdef {
                        ast::TypDef::Extern(typdef) => (typdef.id, TypeDef::Extern),
                        ast::TypDef::Defined(typdef) => {
                            let ast::DefinedTyp {
                                id,
                                tparams,
                                def_typ,
                                ..
                            } = *typdef;
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
        Self {
            global,
            local: Local::default(),
        }
    }

    pub fn localize(&self) -> Self {
        Self::new(self.global)
    }

    pub fn wipe(&self) -> Self {
        Self {
            global: self.global,
            local: Local {
                tdenv: self.local.tdenv.clone(),
                fenv: self.local.fenv.clone(),
                venv: VEnv::new(),
            },
        }
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

    pub fn find_typdef_opt<'a>(&'a self, id: &ast::Id) -> Option<&'a TypeDef> {
        self.local
            .tdenv
            .get(id)
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
            _ => Err(Error::undefined(
                EntityKind::DefinedType,
                id.node.clone(),
                id.span.clone(),
            )),
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
                ast::ParamKind::Def(_, tparams, params, typ) => make::func(
                    tparams.clone(),
                    params.iter().map(param_typ).collect(),
                    typ.clone(),
                ),
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

    // - Expression mapping

    pub fn map_list<I: Interface, E: Extern>(
        &self,
        runner: &mut RunnerContext<'_, Al, I, E>,
        span: &Span,
        vars: &[ast::Var],
        mut eval: impl FnMut(&mut RunnerContext<'_, Al, I, E>, &Self) -> Backtrack<Value>,
    ) -> Backtrack<Vec<Value>> {
        let rows = backtrack_from_result!(self.list_values(runner.arena(), vars), span);
        // Copy handles before the callback can allocate in the arena
        let rows: Vec<_> = rows.into_iter().map(<[Value]>::to_vec).collect();
        let width = rows.first().map_or(0, Vec::len);
        let vars: Vec<_> = vars
            .iter()
            .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
            .collect();
        let mut ctx_sub = self.clone();
        let mut values = Vec::with_capacity(width);
        for column in 0..width {
            for (var, row) in vars.iter().zip(&rows) {
                ctx_sub.add_value(var.clone(), row[column]);
            }
            values.push(backtrack!(eval(runner, &ctx_sub)));
        }
        Backtrack::Ok(values)
    }

    pub fn map_opt<I: Interface, E: Extern>(
        &self,
        runner: &mut RunnerContext<'_, Al, I, E>,
        span: &Span,
        vars: &[ast::Var],
        mut eval: impl FnMut(&mut RunnerContext<'_, Al, I, E>, &Self) -> Backtrack<Value>,
    ) -> Backtrack<Option<Value>> {
        let values = backtrack_from_result!(self.opt_values(runner.arena(), vars), span);
        let Some(values) = values else {
            return Backtrack::Ok(None);
        };
        let mut ctx_sub = self.clone();
        for (var, value) in vars.iter().zip(values) {
            ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
        }
        Backtrack::Ok(Some(backtrack!(eval(runner, &ctx_sub))))
    }

    // - Premise bindings

    pub fn yield_list<I: Interface, E: Extern>(
        mut self,
        runner: &mut RunnerContext<'_, Al, I, E>,
        span: &Span,
        vars_bound: &[ast::Var],
        vars_bind: &[ast::Var],
        mut eval: impl FnMut(&mut RunnerContext<'_, Al, I, E>, Self) -> Backtrack<Self>,
    ) -> Backtrack<Self> {
        let rows = backtrack_from_result!(self.list_values(runner.arena(), vars_bound), span);
        let rows: Vec<_> = rows.into_iter().map(<[Value]>::to_vec).collect();
        let width = rows.first().map_or(0, Vec::len);
        let vars: Vec<_> = vars_bound
            .iter()
            .map(|var| Variable::new(var.id.clone(), var.iters.clone()))
            .collect();
        let mut ctx_sub = self.clone();
        let mut values_bind = vec![Vec::new(); vars_bind.len()];
        for column in 0..width {
            for (var, row) in vars.iter().zip(&rows) {
                ctx_sub.add_value(var.clone(), row[column]);
            }
            // Keep callback writes out of the reusable input context
            let ctx_post = backtrack!(eval(runner, ctx_sub.clone()));
            backtrack!(ctx_post.collect_bindings(vars_bind, &mut values_bind));
        }
        backtrack!(self.bind_iter(runner.arena_mut(), vars_bind, ast::Iter::List, values_bind));
        Backtrack::Ok(self)
    }

    pub fn yield_opt<I: Interface, E: Extern>(
        mut self,
        runner: &mut RunnerContext<'_, Al, I, E>,
        span: &Span,
        vars_bound: &[ast::Var],
        vars_bind: &[ast::Var],
        mut eval: impl FnMut(&mut RunnerContext<'_, Al, I, E>, Self) -> Backtrack<Self>,
    ) -> Backtrack<Self> {
        let values = backtrack_from_result!(self.opt_values(runner.arena(), vars_bound), span);
        let mut values_bind = vec![Vec::new(); vars_bind.len()];
        if let Some(values) = values {
            let mut ctx_sub = self.clone();
            for (var, value) in vars_bound.iter().zip(values) {
                ctx_sub.add_value(Variable::new(var.id.clone(), var.iters.clone()), value);
            }
            let ctx_post = backtrack!(eval(runner, ctx_sub));
            backtrack!(ctx_post.collect_bindings(vars_bind, &mut values_bind));
        }
        backtrack!(self.bind_iter(runner.arena_mut(), vars_bind, ast::Iter::Opt, values_bind));
        Backtrack::Ok(self)
    }

    // - Iteration inputs

    fn opt_values(
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

    fn list_values<'a>(
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

    fn collect_bindings(&self, vars: &[ast::Var], values_bind: &mut [Vec<Value>]) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(values_bind) {
            let var_bound = Variable::new(var.id.clone(), var.iters.clone());
            values.push(*backtrack_from_result!(
                self.find_value(&var_bound),
                &var.id.span
            ));
        }
        Backtrack::Ok(())
    }

    fn bind_iter(
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
                ast::Iter::Opt => make::opt(
                    arena,
                    typ.node.into(),
                    values.into_iter().next(),
                    Span::default(),
                ),
                ast::Iter::List => make::list(arena, typ.node.into(), values, Span::default()),
            };
            let value = backtrack_from_result!(value, &Span::default());
            self.add_value(Variable::new(var.id.clone(), iters), value);
        }
        Backtrack::Ok(())
    }

    // == Environment conversion

    pub fn tdenv(&self) -> TDEnv {
        let mut tdenv = self.global.tdenv.clone();
        tdenv.extend(
            self.local
                .tdenv
                .iter()
                .map(|(id, typdef)| (id.clone(), typdef.clone())),
        );
        tdenv
    }

    pub fn theta_local(&self) -> crate::runtime::ops::typ::Theta {
        let mut theta = crate::runtime::ops::typ::Theta::new();
        for (id, typdef) in self.local.tdenv.iter() {
            if let TypeDef::Defined(tparams, def_typ) = typdef
                && tparams.is_empty()
                && let ast::DefTypKind::Plain(typ) = &def_typ.node
            {
                theta.insert(id.clone(), typ.clone());
            }
        }
        theta
    }
}
