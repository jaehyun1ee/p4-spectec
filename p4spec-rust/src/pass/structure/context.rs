//! Type and metavariable environments used during structuring

use crate::{
    lang::{
        al::ast,
        common::{Id, source::Span},
        data::typ,
    },
    phrase,
    runtime::{
        envs::algo::{MEnv, TDEnv},
        typdef::TypeDef,
    },
};

use super::{StructureError, StructureErrorKind};

/// Definition environments collected from an algorithmic specification
#[derive(Clone, Debug)]
pub struct Context {
    pub(crate) tdenv: TDEnv,
    pub(crate) menv: MEnv,
}

impl Context {
    /// Loads type and metavariable definitions in source order
    pub fn load(spec_al: &ast::Spec) -> Result<Self, StructureError> {
        let mut ctx = Self::init();
        for def_al in spec_al {
            ctx.load_def(def_al)?;
        }
        Ok(ctx)
    }

    fn init() -> Self {
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
            tdenv: TDEnv::new(),
            menv,
        }
    }

    pub fn find_typdef_opt(&self, id: &Id) -> Option<&TypeDef> {
        self.tdenv.get(id)
    }

    pub fn find_typdef(&self, id: &Id) -> Result<&TypeDef, StructureError> {
        self.find_typdef_opt(id)
            .ok_or_else(|| StructureError::new(StructureErrorKind::UndefinedType, id.span.clone()))
    }

    pub fn bound_typdef(&self, id: &Id) -> bool {
        self.find_typdef_opt(id).is_some()
    }

    pub fn find_metavar_opt(&self, id: &Id) -> Option<&ast::Typ> {
        self.menv.get(id)
    }

    pub fn find_metavar(&self, id: &Id) -> Result<&ast::Typ, StructureError> {
        self.find_metavar_opt(id).ok_or_else(|| {
            StructureError::new(StructureErrorKind::UndefinedMetavariable, id.span.clone())
        })
    }

    pub fn bound_metavar(&self, id: &Id) -> bool {
        self.find_metavar_opt(id).is_some()
    }

    fn add_typdef(&mut self, id: Id, typdef: TypeDef) -> Result<(), StructureError> {
        if self.bound_typdef(&id) {
            let error = StructureError::new(StructureErrorKind::DuplicateType, id.span.clone());
            return Err(error);
        }
        self.tdenv.insert(id, typdef);
        Ok(())
    }

    fn add_metavar(&mut self, id: Id, typ: ast::Typ) -> Result<(), StructureError> {
        if self.bound_metavar(&id) {
            let error =
                StructureError::new(StructureErrorKind::DuplicateMetavariable, id.span.clone());
            return Err(error);
        }
        self.menv.insert(id, typ);
        Ok(())
    }

    fn load_def(&mut self, def_al: &ast::Def) -> Result<(), StructureError> {
        match &def_al.node {
            ast::DefKind::Typ(ast::TypDef::Extern(extern_typ_al)) => {
                let id = extern_typ_al.id.clone();
                let typ = typ::make::var(id.clone(), vec![]);
                self.add_metavar(id.clone(), typ)?;
                self.add_typdef(id, TypeDef::Extern)
            }
            ast::DefKind::Typ(ast::TypDef::Defined(defined_typ_al)) => {
                let id = defined_typ_al.id.clone();
                if defined_typ_al.tparams.is_empty() {
                    let typ = typ::make::var(id.clone(), vec![]);
                    self.add_metavar(id.clone(), typ)?;
                }
                let typdef = TypeDef::Defined(
                    defined_typ_al.tparams.clone(),
                    Box::new(defined_typ_al.def_typ.clone()),
                );
                self.add_typdef(id, typdef)
            }
            ast::DefKind::Var(var_def_al) => {
                self.add_metavar(var_def_al.id.clone(), var_def_al.typ.clone())
            }
            _ => Ok(()),
        }
    }
}
