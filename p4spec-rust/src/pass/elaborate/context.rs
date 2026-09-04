//! Elaboration bindings and operation-local fresh state

use crate::{
    lang::{
        common::{Id, ds::set::IdSet, source::Span},
        hints::input::InputHint,
        il::ast,
    },
    phrase,
    runtime::{
        sta::{FEnv, Func, MEnv, REnv, Rel},
        types::{TDEnv, TypeDef, typ},
    },
};

use super::{ElabError, EntityKind};

/// Bindings and fresh state threaded through one elaboration operation
#[derive(Clone, Debug)]
pub(super) struct Context {
    pub(super) frees: IdSet,
    pub(super) tdenv: TDEnv,
    pub(super) menv: MEnv,
    pub(super) renv: REnv,
    pub(super) fenv: FEnv,
}

impl Context {
    // == Constructors

    pub(super) fn new() -> Self {
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

    pub(super) fn find_defined_rel_opt(&self, id: &Id) -> Option<&Rel> {
        self.renv
            .get(id)
            .filter(|rel| matches!(rel, Rel::Defined { .. }))
    }

    pub(super) fn find_defined_rel(&self, id: &Id) -> Result<&Rel, ElabError> {
        self.find_defined_rel_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::DefinedRelation, &id.node, id.span.clone())
        })
    }

    pub(super) fn find_rel_signature_opt(&self, id: &Id) -> Option<(&ast::NotTyp, &InputHint)> {
        match self.renv.get(id)? {
            Rel::Extern {
                not_typ,
                input_hint,
            }
            | Rel::Defined {
                not_typ,
                input_hint,
                ..
            } => Some((not_typ, input_hint)),
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

    fn bound_rule_group(&self, relid: &Id, groupid: &Id) -> bool {
        let Some(Rel::Defined {
            rule_groups,
            else_group,
            ..
        }) = self.find_defined_rel_opt(relid)
        else {
            return false;
        };
        rule_groups
            .iter()
            .any(|group| group.node.0.node == groupid.node)
            || else_group
                .as_ref()
                .is_some_and(|group| group.node.0.node == groupid.node)
    }

    // - Functions

    pub(super) fn find_table_func_opt(&self, id: &Id) -> Option<&Func> {
        self.fenv
            .get(id)
            .filter(|func| matches!(func, Func::Table { .. }))
    }

    pub(super) fn find_table_func(&self, id: &Id) -> Result<&Func, ElabError> {
        self.find_table_func_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::TableFunction, &id.node, id.span.clone())
        })
    }

    pub(super) fn find_defined_func_opt(&self, id: &Id) -> Option<&Func> {
        self.fenv
            .get(id)
            .filter(|func| matches!(func, Func::Defined { .. }))
    }

    pub(super) fn find_defined_func(&self, id: &Id) -> Result<&Func, ElabError> {
        self.find_defined_func_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::DefinedFunction, &id.node, id.span.clone())
        })
    }

    pub(super) fn find_func_signature_opt(
        &self,
        id: &Id,
    ) -> Option<(&[ast::TParam], &[ast::Param], &ast::Typ)> {
        match self.fenv.get(id)? {
            Func::Extern {
                tparams,
                params,
                typ_ret,
            }
            | Func::Builtin {
                tparams,
                params,
                typ_ret,
            }
            | Func::Defined {
                tparams,
                params,
                typ_ret,
                ..
            } => Some((tparams, params, typ_ret)),
            Func::Table {
                params, typ_ret, ..
            } => Some((&[], params, typ_ret)),
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
            return Err(ElabError::duplicate(
                EntityKind::MetaVariable,
                &id.node,
                id.span,
            ));
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

    pub(super) fn add_tparam(&mut self, tparam: ast::TParam) -> Result<(), ElabError> {
        if self.bound_typdef(&tparam) {
            return Err(ElabError::duplicate(
                EntityKind::Type,
                &tparam.node,
                tparam.span,
            ));
        }
        if self.bound_metavar(&tparam) {
            return Err(ElabError::duplicate(
                EntityKind::MetaVariable,
                &tparam.node,
                tparam.span,
            ));
        }
        let typ = typ::var(tparam.clone(), vec![]);
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
        id: Id,
        not_typ: ast::NotTyp,
        input_hint: InputHint,
    ) -> Result<(), ElabError> {
        if self.bound_rel(&id) {
            return Err(ElabError::duplicate(
                EntityKind::Relation,
                &id.node,
                id.span,
            ));
        }
        self.renv.insert(
            id,
            Rel::Extern {
                not_typ: Box::new(not_typ),
                input_hint,
            },
        );
        Ok(())
    }

    pub(super) fn add_defined_rel(
        &mut self,
        id: Id,
        not_typ: ast::NotTyp,
        input_hint: InputHint,
    ) -> Result<(), ElabError> {
        if self.bound_rel(&id) {
            return Err(ElabError::duplicate(
                EntityKind::Relation,
                &id.node,
                id.span,
            ));
        }
        self.renv.insert(
            id,
            Rel::Defined {
                not_typ: Box::new(not_typ),
                input_hint,
                rule_groups: vec![],
                else_group: None,
            },
        );
        Ok(())
    }

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
        let groupid = &rule_group.node.0;
        if self.bound_rule_group(relid, groupid) {
            return Err(ElabError::duplicate(
                EntityKind::RuleGroup,
                &groupid.node,
                groupid.span.clone(),
            ));
        }
        let Rel::Defined { rule_groups, .. } = self.renv.get_mut(relid).expect("defined relation")
        else {
            unreachable!("checked defined relation")
        };
        rule_groups.push(rule_group);
        Ok(())
    }

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
        let groupid = &else_group.node.0;
        if self.bound_rule_group(relid, groupid) {
            return Err(ElabError::duplicate(
                EntityKind::RuleGroup,
                &groupid.node,
                groupid.span.clone(),
            ));
        }
        let Rel::Defined {
            else_group: stored, ..
        } = self.renv.get_mut(relid).expect("defined relation")
        else {
            unreachable!("checked defined relation")
        };
        if stored.is_some() {
            return Err(ElabError::duplicate(
                EntityKind::ElseGroup,
                &relid.node,
                else_group.span,
            ));
        }
        *stored = Some(Box::new(else_group));
        Ok(())
    }

    // - Functions

    pub(super) fn add_extern_func(
        &mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<(), ElabError> {
        self.ensure_func_unbound(&id)?;
        self.fenv.insert(
            id,
            Func::Extern {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
            },
        );
        Ok(())
    }

    pub(super) fn add_builtin_func(
        &mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<(), ElabError> {
        self.ensure_func_unbound(&id)?;
        self.fenv.insert(
            id,
            Func::Builtin {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
            },
        );
        Ok(())
    }

    pub(super) fn add_table_func(
        &mut self,
        id: Id,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<(), ElabError> {
        self.ensure_func_unbound(&id)?;
        self.fenv.insert(
            id,
            Func::Table {
                params,
                typ_ret: Box::new(typ_ret),
                table_rows: vec![],
            },
        );
        Ok(())
    }

    pub(super) fn add_defined_func(
        &mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<(), ElabError> {
        self.ensure_func_unbound(&id)?;
        self.fenv.insert(
            id,
            Func::Defined {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
                clauses: vec![],
                else_clause: None,
            },
        );
        Ok(())
    }

    fn ensure_func_unbound(&self, id: &Id) -> Result<(), ElabError> {
        if self.bound_func(id) {
            return Err(ElabError::duplicate(
                EntityKind::Function,
                &id.node,
                id.span.clone(),
            ));
        }
        Ok(())
    }

    pub(super) fn add_table_func_rows(
        &mut self,
        id: &Id,
        table_rows: Vec<ast::TableRow>,
    ) -> Result<(), ElabError> {
        let Some(Func::Table {
            table_rows: rows_found,
            ..
        }) = self.find_table_func_opt(id)
        else {
            let span = table_rows
                .first()
                .map_or_else(|| id.span.clone(), |row| row.span.clone());
            return Err(ElabError::undefined(
                EntityKind::TableFunction,
                &id.node,
                span,
            ));
        };
        if let Some(row) = rows_found.first() {
            return Err(ElabError::duplicate(
                EntityKind::TableFunction,
                &id.node,
                row.span.clone(),
            ));
        }
        let Func::Table {
            table_rows: stored, ..
        } = self.fenv.get_mut(id).expect("table function")
        else {
            unreachable!("checked table function")
        };
        *stored = table_rows;
        Ok(())
    }

    pub(super) fn add_defined_func_clause(
        &mut self,
        id: &Id,
        clause: ast::Clause,
    ) -> Result<(), ElabError> {
        if self.find_defined_func_opt(id).is_none() {
            return Err(ElabError::undefined(
                EntityKind::Function,
                &id.node,
                clause.span,
            ));
        }
        let Func::Defined { clauses, .. } = self.fenv.get_mut(id).expect("defined function") else {
            unreachable!("checked defined function")
        };
        clauses.push(clause);
        Ok(())
    }

    pub(super) fn add_defined_func_else_clause(
        &mut self,
        id: &Id,
        else_clause: ast::ElseClause,
    ) -> Result<(), ElabError> {
        if self.find_defined_func_opt(id).is_none() {
            return Err(ElabError::undefined(
                EntityKind::Function,
                &id.node,
                else_clause.span,
            ));
        }
        let Func::Defined {
            else_clause: stored,
            ..
        } = self.fenv.get_mut(id).expect("defined function")
        else {
            unreachable!("checked defined function")
        };
        if stored.is_some() {
            return Err(ElabError::duplicate(
                EntityKind::ElseClause,
                &id.node,
                else_clause.span,
            ));
        }
        *stored = Some(Box::new(else_clause));
        Ok(())
    }

    // == Updaters

    // - Type definitions

    pub(super) fn update_typdef(&mut self, id: &Id, typdef: TypeDef) -> Result<(), ElabError> {
        if !self.bound_typdef(id) {
            return Err(ElabError::undefined(
                EntityKind::Type,
                &id.node,
                id.span.clone(),
            ));
        }
        self.tdenv.insert(id.clone(), typdef);
        Ok(())
    }
}
