//! Loaded PL definitions and per-call execution bindings

use std::rc::Rc;

use crate::{
    interp::{
        pl::prepare,
        shared::{
            backtrack::{Backtrack, ok, unwrap_from_result},
            context::{IterContext, ReadContext, WriteContext},
            error::{ContextErrorKind, EntityKind, Error, ErrorKind},
            prepare::ast as exec,
        },
    },
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make},
            var::{SlotIdx, VarSlot},
        },
        pl::ast,
        traits::print::Print,
    },
    runtime::{
        envs::interp::{
            pl::{FEnv, REnv, TDEnv},
            shared::{callable::Callable, frame::Frame},
        },
        typdef::TypeDef,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Local,
}

#[derive(Debug)]
pub struct Global {
    source: ast::Spec,
    tdenv: TDEnv,
    renv: REnv,
    fenv: FEnv,
}

impl Global {
    pub fn load(spec: ast::Spec) -> Result<Self, Error> {
        let mut loaded = Self {
            source: spec.clone(),
            tdenv: TDEnv::new(),
            renv: REnv::new(),
            fenv: FEnv::new(),
        };
        for def in spec {
            match def.node.node {
                ast::DefKind::Typ(typdef) => {
                    let (id, typdef) = match typdef {
                        ast::TypDef::Extern(typdef) => (typdef.id, TypeDef::Extern),
                        ast::TypDef::Defined(typdef) => {
                            let ast::DefinedTyp { id, tparams, def_typ } = *typdef;
                            (id, TypeDef::Defined(tparams, Box::new(def_typ)))
                        }
                    };
                    if loaded.tdenv.contains_key(&id) {
                        return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
                    }
                    loaded.tdenv.insert(id, typdef);
                }
                ast::DefKind::Var(_) => {}
                ast::DefKind::Rel(def) => {
                    let id = match &def {
                        ast::RelDef::Extern(def) => &def.id,
                        ast::RelDef::Defined(def) => &def.id,
                    }
                    .clone();
                    if loaded.renv.contains_key(&id) {
                        return Err(Error::duplicate(
                            EntityKind::Relation,
                            id.node.clone(),
                            id.span.clone(),
                        ));
                    }
                    let callable = Callable { layout: Rc::new(prepare::layout_rel(&def)), def };
                    loaded.renv.insert(id, callable);
                }
                ast::DefKind::MetaFunc(def) => {
                    let id = match &def {
                        ast::MetaFuncDef::Extern(def) => &def.id,
                        ast::MetaFuncDef::Builtin(def) => &def.id,
                        ast::MetaFuncDef::Table(def) => &def.id,
                        ast::MetaFuncDef::Defined(def) => &def.id,
                    }
                    .clone();
                    if loaded.fenv.contains_key(&id) {
                        return Err(Error::duplicate(
                            EntityKind::Function,
                            id.node.clone(),
                            id.span.clone(),
                        ));
                    }
                    let callable = Callable { layout: Rc::new(prepare::layout_func(&def)), def };
                    loaded.fenv.insert(id, Rc::new(callable));
                }
            }
        }
        Ok(loaded)
    }

    /// The annotated PL specification in its original definition order.
    pub fn source(&self) -> &ast::Spec {
        &self.source
    }
}

#[derive(Clone, Debug, Default)]
struct Local {
    tdenv: TDEnv,
    fenv: FEnv,
    frame: Frame,
}

#[derive(Clone, Debug)]
pub struct Context<'global> {
    global: &'global Global,
    local: Local,
}

impl<'global> Context<'global> {
    pub fn new(global: &'global Global) -> Self {
        Self { global, local: Local::default() }
    }

    pub fn localize(&self) -> Self {
        Self::new(self.global)
    }

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

    pub fn layout(&self) -> &crate::runtime::envs::interp::shared::frame::FrameLayout {
        self.local.frame.layout()
    }

    pub fn find_typdef(&self, id: &ast::Id) -> Result<&TypeDef, Error> {
        self.find_typdef_opt(id)
            .ok_or_else(|| Error::undefined(EntityKind::Type, id.node.clone(), id.span.clone()))
    }

    pub fn find_rel(&self, id: &ast::Id) -> Result<&'global Callable<ast::RelDef>, Error> {
        self.global
            .renv
            .get(id)
            .ok_or_else(|| Error::undefined(EntityKind::Relation, id.node.clone(), id.span.clone()))
    }

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

    pub(crate) fn bind_tparam(&mut self, id: ast::Id, typdef: TypeDef) -> Result<(), Error> {
        if self.local.tdenv.contains_key(&id) {
            return Err(Error::duplicate(EntityKind::Type, id.node, id.span));
        }
        self.local.tdenv.insert(id, typdef);
        Ok(())
    }
}

impl ReadContext for Context<'_> {
    type Func = Callable<ast::MetaFuncDef>;

    fn find_value(&self, slot: SlotIdx) -> Option<&Value> {
        self.local.frame.get(slot)
    }
    fn find_iter_var(&self, var: &VarSlot, iter: exec::Iter) -> VarSlot {
        self.local.frame.layout().find_iter_var(var, iter)
    }
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

impl WriteContext for Context<'_> {
    fn add_value(&mut self, slot: SlotIdx, value: Value) {
        self.local.frame.set(slot, value);
    }
    fn clear_value_bindings(&mut self) {
        self.local.frame = self.local.frame.wipe();
    }
    fn add_func(&mut self, id: exec::Id, func: Rc<Self::Func>) -> Result<(), Error> {
        if self.find_func_opt(&id).is_some() {
            return Err(Error::duplicate(EntityKind::Function, id.node, id.span));
        }
        self.local.fenv.insert(id, func);
        Ok(())
    }
}

impl IterContext for Context<'_> {
    fn find_list_values_by_var<'a>(
        &self,
        arena: &'a ValueArena,
        vars: &[exec::Var],
    ) -> Result<Vec<&'a [Value]>, Error> {
        let mut rows = Vec::with_capacity(vars.len());
        for var in vars {
            let value = self.find_value(var.slot).ok_or_else(|| {
                Error::undefined(
                    EntityKind::Value,
                    Print::to_string(&var.var),
                    var.var.id.span.clone(),
                )
            })?;
            rows.push(
                get::list(arena, value)
                    .map_err(|error| Error::from(error).at_if_missing(&var.var.id.span))?,
            );
        }
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
            let value = self.find_value(var.slot).ok_or_else(|| {
                Error::undefined(
                    EntityKind::Value,
                    Print::to_string(&var.var),
                    var.var.id.span.clone(),
                )
            })?;
            values.push(
                get::opt(arena, value)
                    .map_err(|error| Error::from(error).at_if_missing(&var.var.id.span))?,
            );
        }
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

    fn collect_values_by_var(
        &self,
        vars: &[exec::Var],
        columns: &mut [Vec<Value>],
    ) -> Backtrack<()> {
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

    fn bind_list_values_by_var(
        &mut self,
        arena: &mut ValueArena,
        vars: &[exec::Var],
        columns: Vec<Vec<Value>>,
    ) -> Backtrack<()> {
        for (var, values) in vars.iter().zip(columns) {
            let typ = typ::make::iterate(var.var.typ.clone(), &var.var.iters);
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
            let value = unwrap_from_result!(
                make::opt(arena, typ.node.into(), values.into_iter().next(), Span::default()),
                &Span::default()
            );
            self.add_value(var.slot, value);
        }
        ok!(())
    }
}
