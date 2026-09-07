//! Loaded definitions and persistent local execution bindings
//!
//! `Spec::load` owns global definitions. A `Context` contains only local maps:
//! cloning preserves lexical bindings for iteration, while `localize` discards
//! them for a fresh call. Lookups borrow the loaded spec explicitly so neither
//! operation copies or mutates global definitions.

use std::rc::Rc;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::value::{Value, get},
    },
    runtime::{
        envs::{
            interp::VEnv,
            interp_al::{FEnv, REnv, TDEnv},
        },
        typdef::TypeDef,
    },
};

use super::error::{EntityKind, Error, ErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Local,
}

#[derive(Debug)]
pub struct Spec {
    tdenv: TDEnv,
    renv: REnv,
    fenv: FEnv,
}

impl Spec {
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
                    loaded.fenv.insert(id.clone(), func);
                }
            }
        }
        Ok(loaded)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    tdenv: TDEnv,
    fenv: FEnv,
    venv: VEnv,
}

impl Context {
    // == Constructors

    pub fn new() -> Self {
        Self::default()
    }

    pub fn localize(&self) -> Self {
        Self::new()
    }

    pub(super) fn without_values(&self) -> Self {
        Self {
            tdenv: self.tdenv.clone(),
            fenv: self.fenv.clone(),
            venv: VEnv::new(),
        }
    }

    // == Finders

    pub fn find_value_opt(&self, var: &Variable) -> Option<&Rc<Value>> {
        self.venv.get(var)
    }

    pub fn find_value(&self, var: &Variable) -> Result<&Rc<Value>, Error> {
        self.find_value_opt(var).ok_or_else(|| {
            Error::undefined(EntityKind::Value, var.to_string(), var.id.span.clone())
        })
    }

    pub fn find_typdef_opt<'a>(&'a self, spec: &'a Spec, id: &ast::Id) -> Option<&'a TypeDef> {
        self.tdenv.get(id).or_else(|| spec.tdenv.get(id))
    }

    pub fn find_typdef<'a>(&'a self, spec: &'a Spec, id: &ast::Id) -> Result<&'a TypeDef, Error> {
        self.find_typdef_opt(spec, id)
            .ok_or_else(|| Error::undefined(EntityKind::Type, id.node.clone(), id.span.clone()))
    }

    pub fn find_defined_typdef<'a>(
        &'a self,
        spec: &'a Spec,
        id: &ast::Id,
    ) -> Result<(&'a [ast::TParam], &'a ast::DefTyp), Error> {
        match self.find_typdef(spec, id)? {
            TypeDef::Defined(tparams, def_typ) => Ok((tparams, def_typ)),
            _ => Err(Error::undefined(
                EntityKind::DefinedType,
                id.node.clone(),
                id.span.clone(),
            )),
        }
    }

    pub fn find_rel_opt<'a>(&self, spec: &'a Spec, id: &ast::Id) -> Option<&'a ast::RelDef> {
        spec.renv.get(id)
    }

    pub fn find_rel<'a>(&self, spec: &'a Spec, id: &ast::Id) -> Result<&'a ast::RelDef, Error> {
        self.find_rel_opt(spec, id)
            .ok_or_else(|| Error::undefined(EntityKind::Relation, id.node.clone(), id.span.clone()))
    }

    pub fn find_func_opt<'a>(
        &'a self,
        spec: &'a Spec,
        id: &ast::Id,
    ) -> Option<(Scope, &'a ast::MetaFuncDef)> {
        if let Some(func) = self.fenv.get(id) {
            Some((Scope::Local, func))
        } else {
            spec.fenv.get(id).map(|func| (Scope::Global, func))
        }
    }

    pub fn find_func<'a>(
        &'a self,
        spec: &'a Spec,
        id: &ast::Id,
    ) -> Result<(Scope, &'a ast::MetaFuncDef), Error> {
        self.find_func_opt(spec, id)
            .ok_or_else(|| Error::undefined(EntityKind::Function, id.node.clone(), id.span.clone()))
    }

    // == Adders

    pub fn add_value(&mut self, var: Variable, value: Rc<Value>) {
        self.venv.insert(var, value);
    }

    pub fn add_typdef(&mut self, spec: &Spec, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        if self.find_typdef_opt(spec, &id).is_some() {
            return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
        }
        self.tdenv.insert(id, typdef);
        Ok(())
    }

    pub fn add_func(
        &mut self,
        spec: &Spec,
        id: ast::Id,
        func: ast::MetaFuncDef,
    ) -> Result<(), Error> {
        if self.find_func_opt(spec, &id).is_some() {
            return Err(Error::duplicate(EntityKind::Function, id.node, id.span));
        }
        self.fenv.insert(id, func);
        Ok(())
    }

    // == Iteration contexts

    pub fn sub_opt(&self, vars: &[ast::Var]) -> Result<Option<Self>, Error> {
        let mut values = Vec::with_capacity(vars.len());
        for var in vars {
            let mut iters = var.iters.clone();
            iters.push(ast::Iter::Opt);
            let value = self.find_value(&Variable::new(var.id.clone(), iters))?;
            let value =
                get::opt(value).map_err(|error| Error::new(error.into(), var.id.span.clone()))?;
            values.push(value);
        }
        if values.iter().all(|value| value.is_some()) {
            let mut ctx = self.clone();
            for (var, value) in vars.iter().zip(values.into_iter().flatten()) {
                ctx.add_value(
                    Variable::new(var.id.clone(), var.iters.clone()),
                    Rc::clone(value),
                );
            }
            Ok(Some(ctx))
        } else if values.iter().all(|value| value.is_none()) {
            Ok(None)
        } else {
            Err(Error::new(ErrorKind::OptionalityMismatch, Span::default()))
        }
    }

    pub fn sub_list(&self, vars: &[ast::Var]) -> Result<Vec<Self>, Error> {
        let mut rows = Vec::with_capacity(vars.len());
        for var in vars {
            let mut iters = var.iters.clone();
            iters.push(ast::Iter::List);
            let value = self.find_value(&Variable::new(var.id.clone(), iters))?;
            let values =
                get::list(value).map_err(|error| Error::new(error.into(), var.id.span.clone()))?;
            rows.push(values);
        }
        let Some(row) = rows.first() else {
            return Ok(Vec::new());
        };
        let width = row.len();
        for row in &rows {
            if row.len() != width {
                return Err(Error::new(
                    ErrorKind::IterationLengthMismatch {
                        expected: width,
                        actual: row.len(),
                    },
                    Span::default(),
                ));
            }
        }
        let mut ctxs = Vec::with_capacity(width);
        for column in 0..width {
            let mut ctx = self.clone();
            for (var, row) in vars.iter().zip(&rows) {
                ctx.add_value(
                    Variable::new(var.id.clone(), var.iters.clone()),
                    Rc::clone(&row[column]),
                );
            }
            ctxs.push(ctx);
        }
        Ok(ctxs)
    }
}

impl Context {
    pub fn type_env(&self, spec: &Spec) -> TDEnv {
        let mut tdenv = spec.tdenv.clone();
        tdenv.extend(
            self.tdenv
                .iter()
                .map(|(id, typdef)| (id.clone(), typdef.clone())),
        );
        tdenv
    }

    pub fn local_theta(&self) -> crate::runtime::ops::typ::Theta {
        let mut theta = crate::runtime::ops::typ::Theta::new();
        for (id, typdef) in self.tdenv.iter() {
            if let TypeDef::Defined(tparams, def_typ) = typdef
                && tparams.is_empty()
                && let ast::DefTypKind::Plain(typ) = &def_typ.node
            {
                theta.insert(id.clone(), typ.clone());
            }
        }
        theta
    }

    pub fn find_func_typ(
        &self,
        spec: &Spec,
        id: &ast::Id,
    ) -> Result<crate::lang::il::ast::FuncTyp, Error> {
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
        let (_, func) = self.find_func(spec, id)?;
        let (tparams, params, typ): (&[ast::TParam], &[ast::Param], &ast::Typ) = match func {
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
}
