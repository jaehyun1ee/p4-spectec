//! State accumulated while analyzing bindings

use crate::{
    lang::{
        common::{Id, ds::set::IdSet, source::Span},
        data::typ,
        il::ast,
    },
    phrase,
    runtime::{
        env::TDEnv,
        envs::algo::{MEnv, VEnv},
        typdef::TypeDef,
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
            ("bool", typ::make::bool()),
            ("nat", typ::make::nat()),
            ("int", typ::make::int()),
            ("text", typ::make::text()),
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

    pub fn load_def(&mut self, def_al: &ast::Def) {
        match &def_al.node {
            ast::DefKind::Typ(typ_def_al) => self.load_typ_def(typ_def_al),
            ast::DefKind::Var(var_def) => {
                self.menv.insert(var_def.id.clone(), var_def.typ.clone());
            }
            _ => {}
        }
    }

    fn load_typ_def(&mut self, typ_def_al: &ast::TypDef) {
        match typ_def_al {
            ast::TypDef::Extern(extern_typ_al) => {
                self.tdenv.insert(extern_typ_al.id.clone(), TypeDef::Extern);
            }
            ast::TypDef::Defined(defined_typ_al) => {
                let type_def = TypeDef::Defined(
                    defined_typ_al.tparams.clone(),
                    Box::new(defined_typ_al.def_typ.clone()),
                );
                self.tdenv.insert(defined_typ_al.id.clone(), type_def);
            }
        }
    }

    pub fn load_spec(&mut self, spec_al: &ast::Spec) {
        for def_al in spec_al {
            self.load_def(def_al);
        }
    }
}
