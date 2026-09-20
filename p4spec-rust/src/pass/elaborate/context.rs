//! Elaboration bindings and operation-local fresh state
//!
//! `Context` holds the type, meta-variable, relation, and function environments
//! built while walking definitions,
//! plus the free identifiers of the rule or clause under elaboration.
//!
//! A declaration registers an empty definition (`add_defined_rel`),
//! later definitions attach bodies to it (`add_defined_rule_group`),
//! and population takes the completed body back out (`take_defined_rel`).
//!
//! Lookups come in three forms:
//! `find_*_opt` returns an option,
//! `find_*` a located undefined error,
//! and `bound_*` a boolean.

use crate::{
    lang::{
        common::{Id, ds::set::IdSet, source::Span},
        data::typ,
        hints::input::InputHint,
        il::ast,
    },
    phrase,
    runtime::{
        envs::elab::{FEnv, MEnv, REnv, TDEnv},
        typdef::TypeDef,
    },
};

use super::{ElabError, EntityKind};

/// Bindings and fresh state threaded through one elaboration operation.
#[derive(Clone, Debug)]
pub(super) struct Context {
    /// Free identifiers of the rule or clause under elaboration.
    pub(super) frees: IdSet,
    /// Type definitions.
    pub(super) tdenv: TDEnv,
    /// Meta-variable types.
    pub(super) menv: MEnv,
    /// Relation definitions.
    pub(super) renv: REnv,
    /// Function definitions.
    pub(super) fenv: FEnv,
}

impl Context {
    // == Constructors

    /// Creates a context with the primitive meta-variables
    /// `bool`, `nat`, `int`, and `text` bound to their types.
    pub(super) fn new() -> Self {
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
            tdenv: TDEnv::new(),
            menv,
            renv: REnv::new(),
            fenv: FEnv::new(),
        }
    }

    // == Finders

    // - Type definitions

    pub(super) fn find_typdef_opt(&self, id: &Id) -> Option<&TypeDef> {
        self.tdenv.get(id)
    }

    pub(super) fn find_typdef(&self, id: &Id) -> Result<&TypeDef, ElabError> {
        self.find_typdef_opt(id)
            .ok_or_else(|| ElabError::undefined(EntityKind::Type, &id.node, id.span.clone()))
    }

    pub(super) fn bound_typdef(&self, id: &Id) -> bool {
        self.find_typdef_opt(id).is_some()
    }

    // - Meta-variables

    pub(super) fn find_metavar_opt(&self, id: &Id) -> Option<&ast::Typ> {
        self.menv.get(id)
    }

    pub(super) fn bound_metavar(&self, id: &Id) -> bool {
        self.find_metavar_opt(id).is_some()
    }

    // - Relations

    /// Finds a relation only when it is defined rather than extern.
    pub(super) fn find_defined_rel_opt(&self, id: &Id) -> Option<&ast::DefinedRel> {
        match self.renv.get(id)? {
            ast::RelDef::Defined(defined_rel_il) => Some(defined_rel_il),
            ast::RelDef::Extern(_) => None,
        }
    }

    pub(super) fn find_defined_rel(&self, id: &Id) -> Result<&ast::DefinedRel, ElabError> {
        self.find_defined_rel_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::DefinedRelation, &id.node, id.span.clone())
        })
    }

    /// Finds the notation type and input hint of any relation.
    pub(super) fn find_rel_signature_opt(&self, id: &Id) -> Option<(&ast::NotTyp, &InputHint)> {
        match self.renv.get(id)? {
            ast::RelDef::Extern(extern_rel_il) => {
                Some((&extern_rel_il.not_typ, &extern_rel_il.input_hint))
            }
            ast::RelDef::Defined(defined_rel_il) => {
                Some((&defined_rel_il.not_typ, &defined_rel_il.input_hint))
            }
        }
    }

    pub(super) fn find_rel_signature(
        &self,
        id: &Id,
    ) -> Result<(&ast::NotTyp, &InputHint), ElabError> {
        self.find_rel_signature_opt(id)
            .ok_or_else(|| ElabError::undefined(EntityKind::Relation, &id.node, id.span.clone()))
    }

    pub(super) fn bound_rel(&self, id: &Id) -> bool {
        self.find_rel_signature_opt(id).is_some()
    }

    /// Checks whether the relation already has a group with this id.
    fn bound_rule_group(&self, relid: &Id, groupid: &Id) -> bool {
        let Some(defined_rel_il) = self.find_defined_rel_opt(relid) else {
            return false;
        };
        defined_rel_il
            .rule_groups
            .iter()
            .any(|group| group.node.id.node == groupid.node)
            || defined_rel_il
                .else_group
                .as_ref()
                .is_some_and(|group| group.node.id.node == groupid.node)
    }

    // - Functions

    /// Finds a function only when it is a table function.
    pub(super) fn find_table_func_opt(&self, id: &Id) -> Option<&ast::TableFunc> {
        match self.fenv.get(id)? {
            ast::MetaFuncDef::Table(table_func_il) => Some(table_func_il),
            _ => None,
        }
    }

    pub(super) fn find_table_func(&self, id: &Id) -> Result<&ast::TableFunc, ElabError> {
        self.find_table_func_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::TableFunction, &id.node, id.span.clone())
        })
    }

    /// Finds a function only when it is defined by clauses.
    pub(super) fn find_defined_func_opt(&self, id: &Id) -> Option<&ast::DefinedFunc> {
        match self.fenv.get(id)? {
            ast::MetaFuncDef::Defined(defined_func_il) => Some(defined_func_il),
            _ => None,
        }
    }

    pub(super) fn find_defined_func(&self, id: &Id) -> Result<&ast::DefinedFunc, ElabError> {
        self.find_defined_func_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::DefinedFunction, &id.node, id.span.clone())
        })
    }

    /// Finds the type parameters, parameters, and return type of any function.
    pub(super) fn find_func_signature_opt(
        &self,
        id: &Id,
    ) -> Option<(&[ast::TParam], &[ast::Param], &ast::Typ)> {
        match self.fenv.get(id)? {
            ast::MetaFuncDef::Extern(extern_func_il) => {
                Some((&extern_func_il.tparams, &extern_func_il.params, &extern_func_il.typ))
            }
            ast::MetaFuncDef::Builtin(builtin_func_il) => {
                Some((&builtin_func_il.tparams, &builtin_func_il.params, &builtin_func_il.typ))
            }
            // Table functions take no type parameters
            ast::MetaFuncDef::Table(table_func_il) => {
                Some((&[], &table_func_il.params, &table_func_il.typ))
            }
            ast::MetaFuncDef::Defined(defined_func_il) => {
                Some((&defined_func_il.tparams, &defined_func_il.params, &defined_func_il.typ))
            }
        }
    }

    pub(super) fn find_func_signature(
        &self,
        id: &Id,
    ) -> Result<(&[ast::TParam], &[ast::Param], &ast::Typ), ElabError> {
        self.find_func_signature_opt(id)
            .ok_or_else(|| ElabError::undefined(EntityKind::Function, &id.node, id.span.clone()))
    }

    pub(super) fn bound_func(&self, id: &Id) -> bool {
        self.find_func_signature_opt(id).is_some()
    }

    // == Adders

    // - Free variables

    pub(super) fn add_free(&mut self, id: Id) {
        self.frees.insert(id);
    }

    pub(super) fn add_frees(&mut self, ids: &IdSet) {
        for id in ids.iter().cloned() {
            self.add_free(id);
        }
    }

    pub(super) fn reset_frees(&mut self) {
        self.frees = IdSet::new();
    }

    // - Meta-variables

    pub(super) fn add_metavar(&mut self, id: Id, typ: ast::Typ) -> Result<(), ElabError> {
        if self.bound_metavar(&id) {
            return Err(ElabError::duplicate(EntityKind::MetaVariable, &id.node, id.span));
        }
        self.menv.insert(id, typ);
        Ok(())
    }

    // - Type definitions

    pub(super) fn add_typdef(&mut self, id: Id, typdef: TypeDef) -> Result<(), ElabError> {
        if self.bound_typdef(&id) {
            return Err(ElabError::duplicate(EntityKind::Type, &id.node, id.span));
        }
        self.tdenv.insert(id, typdef);
        Ok(())
    }

    /// Binds a type parameter as a type and as a meta-variable of that type.
    pub(super) fn add_tparam(&mut self, tparam: ast::TParam) -> Result<(), ElabError> {
        if self.bound_typdef(&tparam) {
            return Err(ElabError::duplicate(EntityKind::Type, &tparam.node, tparam.span));
        }
        if self.bound_metavar(&tparam) {
            return Err(ElabError::duplicate(EntityKind::MetaVariable, &tparam.node, tparam.span));
        }
        let typ = typ::make::var(tparam.clone(), vec![]);
        self.add_typdef(tparam.clone(), TypeDef::Parameter)?;
        self.add_metavar(tparam, typ)
    }

    pub(super) fn add_tparams(&mut self, tparams: &[ast::TParam]) -> Result<(), ElabError> {
        for tparam in tparams {
            self.add_tparam(tparam.clone())?;
        }
        Ok(())
    }

    // - Relations

    pub(super) fn add_extern_rel(
        &mut self,
        extern_rel_il: ast::ExternRel,
    ) -> Result<(), ElabError> {
        let id = extern_rel_il.id.clone();
        if self.bound_rel(&id) {
            return Err(ElabError::duplicate(EntityKind::Relation, &id.node, id.span));
        }
        self.renv
            .insert(id, ast::RelDef::Extern(Box::new(extern_rel_il)));
        Ok(())
    }

    pub(super) fn add_defined_rel(
        &mut self,
        defined_rel_il: ast::DefinedRel,
    ) -> Result<(), ElabError> {
        let id = defined_rel_il.id.clone();
        if self.bound_rel(&id) {
            return Err(ElabError::duplicate(EntityKind::Relation, &id.node, id.span));
        }
        self.renv
            .insert(id, ast::RelDef::Defined(Box::new(defined_rel_il)));
        Ok(())
    }

    /// Attaches an elaborated rule group to its defined relation.
    pub(super) fn add_defined_rule_group(
        &mut self,
        relid: &Id,
        rule_group: ast::RuleGroup,
    ) -> Result<(), ElabError> {
        if self.find_defined_rel_opt(relid).is_none() {
            return Err(ElabError::undefined(
                EntityKind::Relation,
                &relid.node,
                relid.span.clone(),
            ));
        }
        // Group ids are unique within a relation
        let groupid = &rule_group.node.id;
        if self.bound_rule_group(relid, groupid) {
            return Err(ElabError::duplicate(
                EntityKind::RuleGroup,
                &groupid.node,
                groupid.span.clone(),
            ));
        }
        let ast::RelDef::Defined(defined_rel_il) =
            self.renv.get_mut(relid).expect("defined relation")
        else {
            unreachable!("checked defined relation")
        };
        defined_rel_il.rule_groups.push(rule_group);
        Ok(())
    }

    /// Attaches the single otherwise group to its defined relation.
    pub(super) fn add_defined_else_group(
        &mut self,
        relid: &Id,
        else_group: ast::ElseGroup,
    ) -> Result<(), ElabError> {
        if self.find_defined_rel_opt(relid).is_none() {
            return Err(ElabError::undefined(
                EntityKind::Relation,
                &relid.node,
                relid.span.clone(),
            ));
        }
        let groupid = &else_group.node.id;
        if self.bound_rule_group(relid, groupid) {
            return Err(ElabError::duplicate(
                EntityKind::RuleGroup,
                &groupid.node,
                groupid.span.clone(),
            ));
        }
        let ast::RelDef::Defined(defined_rel_il) =
            self.renv.get_mut(relid).expect("defined relation")
        else {
            unreachable!("checked defined relation")
        };
        // A relation has at most one otherwise group
        if defined_rel_il.else_group.is_some() {
            return Err(ElabError::duplicate(EntityKind::ElseGroup, &relid.node, else_group.span));
        }
        defined_rel_il.else_group = Some(else_group);
        Ok(())
    }

    // - Functions

    pub(super) fn add_extern_func(
        &mut self,
        extern_func_il: ast::ExternFunc,
    ) -> Result<(), ElabError> {
        let id = extern_func_il.id.clone();
        self.ensure_func_unbound(&id)?;
        self.fenv
            .insert(id, ast::MetaFuncDef::Extern(extern_func_il));
        Ok(())
    }

    pub(super) fn add_builtin_func(
        &mut self,
        builtin_func_il: ast::BuiltinFunc,
    ) -> Result<(), ElabError> {
        let id = builtin_func_il.id.clone();
        self.ensure_func_unbound(&id)?;
        self.fenv
            .insert(id, ast::MetaFuncDef::Builtin(builtin_func_il));
        Ok(())
    }

    pub(super) fn add_table_func(
        &mut self,
        table_func_il: ast::TableFunc,
    ) -> Result<(), ElabError> {
        let id = table_func_il.id.clone();
        self.ensure_func_unbound(&id)?;
        self.fenv.insert(id, ast::MetaFuncDef::Table(table_func_il));
        Ok(())
    }

    pub(super) fn add_defined_func(
        &mut self,
        defined_func_il: ast::DefinedFunc,
    ) -> Result<(), ElabError> {
        let id = defined_func_il.id.clone();
        self.ensure_func_unbound(&id)?;
        self.fenv
            .insert(id, ast::MetaFuncDef::Defined(Box::new(defined_func_il)));
        Ok(())
    }

    /// Rejects a function id that any function kind already uses.
    fn ensure_func_unbound(&self, id: &Id) -> Result<(), ElabError> {
        if self.bound_func(id) {
            return Err(ElabError::duplicate(EntityKind::Function, &id.node, id.span.clone()));
        }
        Ok(())
    }

    /// Stores the rows of a declared table function, which must still be empty.
    pub(super) fn add_table_func_rows(
        &mut self,
        id: &Id,
        table_rows: Vec<ast::TableRow>,
    ) -> Result<(), ElabError> {
        let Some(table_func_il) = self.find_table_func_opt(id) else {
            let span = table_rows
                .first()
                .map_or_else(|| id.span.clone(), |row| row.span.clone());
            return Err(ElabError::undefined(EntityKind::TableFunction, &id.node, span));
        };
        // Report a second table body at its first row
        if let Some(row) = table_func_il.rows.first() {
            return Err(ElabError::duplicate(
                EntityKind::TableFunction,
                &id.node,
                row.span.clone(),
            ));
        }
        let ast::MetaFuncDef::Table(table_func_il) = self.fenv.get_mut(id).expect("table function")
        else {
            unreachable!("checked table function")
        };
        table_func_il.rows = table_rows;
        Ok(())
    }

    /// Appends a clause to a declared function.
    pub(super) fn add_defined_func_clause(
        &mut self,
        id: &Id,
        clause: ast::Clause,
    ) -> Result<(), ElabError> {
        if self.find_defined_func_opt(id).is_none() {
            return Err(ElabError::undefined(EntityKind::Function, &id.node, clause.span));
        }
        let ast::MetaFuncDef::Defined(defined_func_il) =
            self.fenv.get_mut(id).expect("defined function")
        else {
            unreachable!("checked defined function")
        };
        defined_func_il.clauses.push(clause);
        Ok(())
    }

    /// Sets the single otherwise clause of a declared function.
    pub(super) fn add_defined_func_else_clause(
        &mut self,
        id: &Id,
        else_clause: ast::ElseClause,
    ) -> Result<(), ElabError> {
        if self.find_defined_func_opt(id).is_none() {
            return Err(ElabError::undefined(EntityKind::Function, &id.node, else_clause.span));
        }
        let ast::MetaFuncDef::Defined(defined_func_il) =
            self.fenv.get_mut(id).expect("defined function")
        else {
            unreachable!("checked defined function")
        };
        // A function has at most one otherwise clause
        if defined_func_il.else_clause.is_some() {
            return Err(ElabError::duplicate(EntityKind::ElseClause, &id.node, else_clause.span));
        }
        defined_func_il.else_clause = Some(else_clause);
        Ok(())
    }

    // == Definition extraction

    // - Relations

    /// Removes a defined relation and returns it with its rule groups.
    pub(super) fn take_defined_rel(&mut self, id: &Id) -> Result<ast::DefinedRel, ElabError> {
        self.find_defined_rel(id)?;
        let Some(ast::RelDef::Defined(defined_rel_il)) = self.renv.remove(id) else {
            unreachable!("checked defined relation")
        };
        Ok(*defined_rel_il)
    }

    // - Functions

    /// Removes a table function and returns it with its collected rows.
    pub(super) fn take_table_func(&mut self, id: &Id) -> Result<ast::TableFunc, ElabError> {
        self.find_table_func(id)?;
        let Some(ast::MetaFuncDef::Table(table_func_il)) = self.fenv.remove(id) else {
            unreachable!("checked table function")
        };
        Ok(table_func_il)
    }

    /// Removes a defined function and returns it with its collected clauses.
    pub(super) fn take_defined_func(&mut self, id: &Id) -> Result<ast::DefinedFunc, ElabError> {
        self.find_defined_func(id)?;
        let Some(ast::MetaFuncDef::Defined(defined_func_il)) = self.fenv.remove(id) else {
            unreachable!("checked defined function")
        };
        Ok(*defined_func_il)
    }

    // == Updaters

    // - Type definitions

    /// Replaces the definition of an already-declared type.
    pub(super) fn update_typdef(&mut self, id: &Id, typdef: TypeDef) -> Result<(), ElabError> {
        if !self.bound_typdef(id) {
            return Err(ElabError::undefined(EntityKind::Type, &id.node, id.span.clone()));
        }
        self.tdenv.insert(id.clone(), typdef);
        Ok(())
    }
}
