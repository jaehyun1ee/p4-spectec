//! Type and metavariable environments used during structuring
//!
//! `Context::load` reads every type and meta-variable definition of the AL
//! specification once;
//! structuring then only queries `tdenv` and `menv`,
//! for example to expand a variant type when totalizing a case analysis
//! or to pick fresh input names from meta-variable types.

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

/// Type and meta-variable environments of the specification being structured.
#[derive(Clone, Debug)]
pub struct Context {
    /// Type definitions.
    pub(crate) tdenv: TDEnv,
    /// Meta-variable types.
    pub(crate) menv: MEnv,
}

impl Context {
    // - Constructor

    /// Creates a context with the primitive meta-variables bound.
    fn init() -> Self {
        let mut menv = MEnv::new();
        for (text_name, typ) in [
            ("bool", typ::make::bool()),
            ("nat", typ::make::nat()),
            ("int", typ::make::int()),
            ("text", typ::make::text()),
        ] {
            let id = phrase!(node: text_name.to_owned(), span: Span::default());
            menv.insert(id, typ);
        }
        Self { tdenv: TDEnv::new(), menv }
    }

    // - Adders

    fn add_typdef(&mut self, id: Id, typdef: TypeDef) -> Result<(), StructureError> {
        if self.tdenv.contains_key(&id) {
            let error = StructureError::new(StructureErrorKind::DuplicateType, id.span.clone());
            return Err(error);
        }
        self.tdenv.insert(id, typdef);
        Ok(())
    }

    fn add_metavar(&mut self, id: Id, typ: ast::Typ) -> Result<(), StructureError> {
        if self.menv.contains_key(&id) {
            let error =
                StructureError::new(StructureErrorKind::DuplicateMetavariable, id.span.clone());
            return Err(error);
        }
        self.menv.insert(id, typ);
        Ok(())
    }

    // - Definition loading

    /// Loads a type or meta-variable definition; other definitions add nothing.
    fn load_def(&mut self, def_al: &ast::Def) -> Result<(), StructureError> {
        let def_kind_al = &def_al.node;
        match def_kind_al {
            ast::DefKind::Typ(typ_def_al) => self.load_typ_def(typ_def_al),
            ast::DefKind::Var(def_var_al) => self.load_var_def(def_var_al),
            _ => Ok(()),
        }
    }

    fn load_typ_def(&mut self, typ_def_al: &ast::TypDef) -> Result<(), StructureError> {
        match typ_def_al {
            ast::TypDef::Extern(extern_typ_al) => self.load_extern_typ(extern_typ_al),
            ast::TypDef::Defined(defined_typ_al) => self.load_defined_typ(defined_typ_al),
        }
    }

    /// Registers an extern type and a meta-variable of that type.
    fn load_extern_typ(&mut self, extern_typ_al: &ast::ExternTyp) -> Result<(), StructureError> {
        let id = extern_typ_al.id.clone();
        let typ = typ::make::var(id.clone(), vec![]);
        self.add_metavar(id.clone(), typ)?;
        self.add_typdef(id, TypeDef::Extern)
    }

    /// Registers a defined type and, if unparameterized, a meta-variable of it.
    fn load_defined_typ(&mut self, defined_typ_al: &ast::DefinedTyp) -> Result<(), StructureError> {
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

    fn load_var_def(&mut self, def_var_al: &ast::VarDef) -> Result<(), StructureError> {
        self.add_metavar(def_var_al.id.clone(), def_var_al.typ.clone())
    }

    /// Builds the context from every definition of the specification.
    pub fn load(spec_al: &ast::Spec) -> Result<Self, StructureError> {
        let mut ctx = Self::init();
        for def_al in spec_al {
            ctx.load_def(def_al)?;
        }
        Ok(ctx)
    }
}
