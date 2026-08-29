//! State accumulated while analyzing bindings

use crate::{
    lang::{
        common::{Id, ds::set::IdSet},
        il::ast,
    },
    runtime::{
        sta::{MEnv, VEnv},
        types::{TDEnv, TypeDef, typ},
    },
    spanned_default,
};

use super::super::{AlgoError, AlgoErrorKind};

/// Environments threaded through binding analysis
#[derive(Clone, Debug)]
pub struct Context {
    pub frees: IdSet,
    pub venv: VEnv,
    pub tdenv: TDEnv,
    pub menv: MEnv,
}

impl Context {
    pub fn new() -> Self {
        let mut menv = MEnv::new();
        for (name, typ) in [
            ("bool", typ::bool()),
            ("nat", typ::nat()),
            ("int", typ::int()),
            ("text", typ::text()),
        ] {
            let id = spanned_default!(node: name.to_owned());
            menv.insert(id, typ);
        }
        Self {
            frees: IdSet::new(),
            venv: VEnv::new(),
            tdenv: TDEnv::new(),
            menv,
        }
    }

    pub fn add_free(&mut self, id: Id) {
        self.frees.insert(id);
    }

    pub fn add_frees(&mut self, ids: &IdSet) {
        self.frees.extend(ids.iter().cloned());
    }

    /// Adds bounds without replacing bindings already present in this context
    pub fn add_bounds(&mut self, venv: &VEnv) {
        for (id, dim) in venv.iter() {
            if !self.venv.contains_key(id) {
                self.venv.insert(id.clone(), dim.clone());
            }
        }
    }

    pub fn find_typdef_opt(&self, id: &Id) -> Option<&TypeDef> {
        self.tdenv.get(id)
    }

    pub fn find_typdef(&self, id: &Id) -> Result<&TypeDef, AlgoError> {
        self.find_typdef_opt(id)
            .ok_or_else(|| AlgoError::new(AlgoErrorKind::UndefinedType, id.span.clone()))
    }

    pub fn load_def(&mut self, def: &ast::Def) {
        match &def.node {
            ast::DefKind::ExternTyp(typ_def) => {
                self.tdenv.insert(typ_def.id.clone(), TypeDef::Extern);
            }
            ast::DefKind::Typ(typ_def) => {
                let value =
                    TypeDef::Defined(typ_def.tparams.clone(), Box::new(typ_def.def_typ.clone()));
                self.tdenv.insert(typ_def.id.clone(), value);
            }
            ast::DefKind::Var(var_def) => {
                self.menv.insert(var_def.id.clone(), var_def.typ.clone());
            }
            _ => {}
        }
    }

    pub fn load_spec(&mut self, spec: &ast::Spec) {
        for def in spec {
            self.load_def(def);
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
