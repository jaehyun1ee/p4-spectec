//! State accumulated while analyzing bindings

use crate::{
    lang::{
        common::{Id, ds::set::IdSet, source::Span},
        il::ast,
    },
    phrase,
    runtime::{
        sta::{MEnv, VEnv},
        types::{TDEnv, TypeDef, typ},
    },
};

use super::super::{AlgoError, AlgoErrorKind};

/// Environments and fresh state threaded through binding analysis
#[derive(Clone, Debug)]
pub struct Context {
    pub(crate) frees: IdSet,
    pub(crate) venv: VEnv,
    pub(crate) tdenv: TDEnv,
    pub(crate) menv: MEnv,
}

impl Context {
    // == Constructors

    pub fn new() -> Self {
        let mut menv = MEnv::new();
        for (name, typ) in [
            ("bool", typ::bool()),
            ("nat", typ::nat()),
            ("int", typ::int()),
            ("text", typ::text()),
        ] {
            let id = phrase!(node: name.to_owned(), span: Span::default());
            menv.insert(id, typ);
        }
        Self {
            frees: IdSet::new(),
            venv: VEnv::new(),
            tdenv: TDEnv::new(),
            menv,
        }
    }

    // == Adders

    pub fn add_free(&mut self, id: Id) {
        self.frees.insert(id);
    }

    pub fn add_frees(&mut self, ids: &IdSet) {
        for id in ids.iter() {
            self.add_free(id.clone());
        }
    }

    pub fn add_bounds(&mut self, venv: &VEnv) {
        for (id, dim) in venv.iter() {
            if !self.venv.contains_key(id) {
                self.venv.insert(id.clone(), dim.clone());
            }
        }
    }

    // == Finders

    pub fn find_typdef_opt(&self, id: &Id) -> Option<&TypeDef> {
        self.tdenv.get(id)
    }

    pub fn find_typdef(&self, id: &Id) -> Result<&TypeDef, AlgoError> {
        self.find_typdef_opt(id)
            .ok_or_else(|| AlgoError::new(AlgoErrorKind::UndefinedType, id.span.clone()))
    }

    // == Definition loading

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
