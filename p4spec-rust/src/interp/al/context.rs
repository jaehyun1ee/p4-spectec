//! Loaded definitions and persistent local execution bindings
//!
//! `Global::load` owns global definitions. A `Context` borrows them and owns
//! persistent local bindings. Cloning preserves the local scope; `localize`
//! starts a fresh scope while retaining the same global definitions.

use crate::interp::al::error::ContextErrorKind;

use crate::{
    lang::{
        al::ast,
        common::{Variable, source::Span},
        data::value::{Value, ValueArena, get},
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
                    loaded.fenv.insert(id.clone(), func);
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

    pub fn find_func_opt<'a>(&'a self, id: &ast::Id) -> Option<(Scope, &'a ast::MetaFuncDef)> {
        if let Some(func) = self.local.fenv.get(id) {
            Some((Scope::Local, func))
        } else {
            self.global.fenv.get(id).map(|func| (Scope::Global, func))
        }
    }

    pub fn find_func<'a>(&'a self, id: &ast::Id) -> Result<(Scope, &'a ast::MetaFuncDef), Error> {
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

    pub fn add_func(&mut self, id: ast::Id, func: ast::MetaFuncDef) -> Result<(), Error> {
        if self.find_func_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Function, id.node, id.span));
        }
        self.local.fenv.insert(id, func);
        Ok(())
    }

    // == Iteration contexts

    pub fn sub_opt(&self, arena: &ValueArena, vars: &[ast::Var]) -> Result<Option<Self>, Error> {
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
            let mut ctx = self.clone();
            for (var, value) in vars.iter().zip(values.into_iter().flatten()) {
                ctx.add_value(Variable::new(var.id.clone(), var.iters.clone()), *value);
            }
            Ok(Some(ctx))
        } else if values.iter().all(|value| value.is_none()) {
            Ok(None)
        } else {
            Err(Error::new(
                ErrorKind::Context(ContextErrorKind::OptionalityMismatch),
                Span::default(),
            ))
        }
    }

    pub fn sub_list(&self, arena: &ValueArena, vars: &[ast::Var]) -> Result<Vec<Self>, Error> {
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
        let mut ctxs = Vec::with_capacity(width);
        for column in 0..width {
            let mut ctx = self.clone();
            for (var, row) in vars.iter().zip(&rows) {
                ctx.add_value(
                    Variable::new(var.id.clone(), var.iters.clone()),
                    row[column],
                );
            }
            ctxs.push(ctx);
        }
        Ok(ctxs)
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
