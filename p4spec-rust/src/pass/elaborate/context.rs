//! Transactional elaboration bindings and operation-local fresh state

use std::ops::{Deref, DerefMut};

use crate::{
    lang::{
        common::{Id, ds::set::IdSet},
        hints::input::InputHint,
        il::ast,
    },
    runtime::{
        sta::{FEnv, Func, MEnv, REnv, Rel},
        types::{TDEnv, TypeDef, typ},
    },
    spanned_default,
};

use super::{ElabError, EntityKind};

type DefinedRelState<'a> = (
    &'a ast::NotTyp,
    &'a InputHint,
    &'a [ast::RuleGroup],
    Option<&'a ast::ElseGroup>,
);
type RelSignature<'a> = (&'a ast::NotTyp, &'a InputHint);
type TableFuncState<'a> = (&'a [ast::Param], &'a ast::Typ, &'a [ast::TableRow]);
type DefinedFuncState<'a> = (
    &'a [ast::TParam],
    &'a [ast::Param],
    &'a ast::Typ,
    &'a [ast::Clause],
    Option<&'a ast::ElseClause>,
);
type FuncSignature<'a> = (&'a [ast::TParam], &'a [ast::Param], &'a ast::Typ);

/// Bindings and fresh state threaded through one elaboration operation
#[derive(Debug)]
pub(super) struct Context {
    pub(super) frees: IdSet,
    pub(super) tdenv: TDEnv,
    pub(super) menv: MEnv,
    pub(super) renv: REnv,
    pub(super) fenv: FEnv,
    undo: Vec<Undo>,
    checkpoints: Vec<usize>,
}

#[derive(Debug)]
enum Undo {
    RemoveFree(Id),
    RestoreFrees(IdSet),
    RemoveTypDef(Id),
    RestoreTypDef(Id, TypeDef),
    RemoveMetavar(Id),
    RemoveRel(Id),
    PopRuleGroup(Id),
    ClearElseGroup(Id),
    RemoveFunc(Id),
    ClearTableRows(Id),
    PopClause(Id),
    ClearElseClause(Id),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Checkpoint {
    depth: usize,
    undo_len: usize,
}

pub(super) struct Scope<'a> {
    ctx: &'a mut Context,
    checkpoint: Checkpoint,
}

impl Deref for Scope<'_> {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl DerefMut for Scope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        self.ctx.rollback_scope(self.checkpoint);
    }
}

impl Context {
    pub(super) fn new() -> Self {
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
            tdenv: TDEnv::new(),
            menv,
            renv: REnv::new(),
            fenv: FEnv::new(),
            undo: vec![],
            checkpoints: vec![],
        }
    }

    pub(super) fn checkpoint(&mut self) -> Checkpoint {
        let checkpoint = Checkpoint {
            depth: self.checkpoints.len(),
            undo_len: self.undo.len(),
        };
        self.checkpoints.push(checkpoint.undo_len);
        checkpoint
    }

    fn assert_innermost(&self, checkpoint: Checkpoint) {
        assert_eq!(checkpoint.depth + 1, self.checkpoints.len());
        assert_eq!(Some(&checkpoint.undo_len), self.checkpoints.last());
    }

    pub(super) fn commit(&mut self, checkpoint: Checkpoint) {
        self.assert_innermost(checkpoint);
        self.checkpoints.pop();
        if self.checkpoints.is_empty() {
            self.undo.clear();
        }
    }

    pub(super) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.assert_innermost(checkpoint);
        self.checkpoints.pop();
        while self.undo.len() > checkpoint.undo_len {
            match self.undo.pop().expect("undo entry") {
                Undo::RemoveFree(id) => {
                    self.frees.take(&id).expect("recorded free binding");
                }
                Undo::RestoreFrees(frees) => self.frees = frees,
                Undo::RemoveTypDef(id) => {
                    self.tdenv.remove(&id).expect("recorded type binding");
                }
                Undo::RestoreTypDef(id, typdef) => {
                    self.tdenv.insert(id, typdef);
                }
                Undo::RemoveMetavar(id) => {
                    self.menv
                        .remove(&id)
                        .expect("recorded metavariable binding");
                }
                Undo::RemoveRel(id) => {
                    self.renv.remove(&id).expect("recorded relation binding");
                }
                Undo::PopRuleGroup(id) => {
                    let Rel::Defined { rule_groups, .. } =
                        self.renv.get_mut(&id).expect("recorded defined relation")
                    else {
                        unreachable!("recorded defined relation")
                    };
                    rule_groups.pop().expect("recorded rule group");
                }
                Undo::ClearElseGroup(id) => {
                    let Rel::Defined { else_group, .. } =
                        self.renv.get_mut(&id).expect("recorded defined relation")
                    else {
                        unreachable!("recorded defined relation")
                    };
                    else_group.take().expect("recorded else group");
                }
                Undo::RemoveFunc(id) => {
                    self.fenv.remove(&id).expect("recorded function binding");
                }
                Undo::ClearTableRows(id) => {
                    let Func::Table { table_rows, .. } =
                        self.fenv.get_mut(&id).expect("recorded table function")
                    else {
                        unreachable!("recorded table function")
                    };
                    table_rows.clear();
                }
                Undo::PopClause(id) => {
                    let Func::Defined { clauses, .. } =
                        self.fenv.get_mut(&id).expect("recorded defined function")
                    else {
                        unreachable!("recorded defined function")
                    };
                    clauses.pop().expect("recorded function clause");
                }
                Undo::ClearElseClause(id) => {
                    let Func::Defined { else_clause, .. } =
                        self.fenv.get_mut(&id).expect("recorded defined function")
                    else {
                        unreachable!("recorded defined function")
                    };
                    else_clause.take().expect("recorded else clause");
                }
            }
        }
    }

    fn rollback_scope(&mut self, checkpoint: Checkpoint) {
        if std::thread::panicking() && self.checkpoints.len() > checkpoint.depth + 1 {
            // The unwinding operation may still own nested checkpoints
            self.checkpoints.truncate(checkpoint.depth + 1);
        }
        self.rollback(checkpoint);
    }

    pub(super) fn scope(&mut self) -> Scope<'_> {
        let checkpoint = self.checkpoint();
        Scope {
            ctx: self,
            checkpoint,
        }
    }

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

    pub(super) fn find_metavar_opt(&self, id: &Id) -> Option<&ast::Typ> {
        self.menv.get(id)
    }

    pub(super) fn bound_metavar(&self, id: &Id) -> bool {
        self.find_metavar_opt(id).is_some()
    }

    pub(super) fn add_free(&mut self, id: Id) {
        if self.frees.insert(id.clone()) && !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveFree(id));
        }
    }

    pub(super) fn add_frees(&mut self, ids: &IdSet) {
        for id in ids.iter().cloned() {
            self.add_free(id);
        }
    }

    pub(super) fn reset_frees(&mut self) {
        let frees = std::mem::take(&mut self.frees);
        if !self.checkpoints.is_empty() && !frees.is_empty() {
            self.undo.push(Undo::RestoreFrees(frees));
        }
    }

    pub(super) fn add_metavar(&mut self, id: Id, typ: ast::Typ) -> Result<(), ElabError> {
        if self.bound_metavar(&id) {
            return Err(ElabError::duplicate(
                EntityKind::MetaVariable,
                &id.node,
                id.span,
            ));
        }
        self.menv.insert(id.clone(), typ);
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveMetavar(id));
        }
        Ok(())
    }

    pub(super) fn add_typdef(&mut self, id: Id, typdef: TypeDef) -> Result<(), ElabError> {
        if self.bound_typdef(&id) {
            return Err(ElabError::duplicate(EntityKind::Type, &id.node, id.span));
        }
        self.tdenv.insert(id.clone(), typdef);
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveTypDef(id));
        }
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

    pub(super) fn update_typdef(&mut self, id: &Id, typdef: TypeDef) -> Result<(), ElabError> {
        if !self.bound_typdef(id) {
            return Err(ElabError::undefined(
                EntityKind::Type,
                &id.node,
                id.span.clone(),
            ));
        }
        let previous = self
            .tdenv
            .insert(id.clone(), typdef)
            .expect("checked type binding");
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RestoreTypDef(id.clone(), previous));
        }
        Ok(())
    }

    pub(super) fn find_defined_rel_opt(&self, id: &Id) -> Option<DefinedRelState<'_>> {
        match self.renv.get(id)? {
            Rel::Defined {
                not_typ,
                input_hint,
                rule_groups,
                else_group,
            } => Some((not_typ, input_hint, rule_groups, else_group.as_deref())),
            Rel::Extern { .. } => None,
        }
    }

    pub(super) fn find_defined_rel(&self, id: &Id) -> Result<DefinedRelState<'_>, ElabError> {
        self.find_defined_rel_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::DefinedRelation, &id.node, id.span.clone())
        })
    }

    pub(super) fn find_rel_signature_opt(&self, id: &Id) -> Option<RelSignature<'_>> {
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

    pub(super) fn find_rel_signature(&self, id: &Id) -> Result<RelSignature<'_>, ElabError> {
        self.find_rel_signature_opt(id)
            .ok_or_else(|| ElabError::undefined(EntityKind::Relation, &id.node, id.span.clone()))
    }

    pub(super) fn bound_rel(&self, id: &Id) -> bool {
        self.find_rel_signature_opt(id).is_some()
    }

    fn bound_rule_group(&self, relid: &Id, groupid: &Id) -> bool {
        let Some((_, _, groups, else_group)) = self.find_defined_rel_opt(relid) else {
            return false;
        };
        groups.iter().any(|group| group.node.0.node == groupid.node)
            || else_group.is_some_and(|group| group.node.0.node == groupid.node)
    }

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
            id.clone(),
            Rel::Extern {
                not_typ: Box::new(not_typ),
                input_hint,
            },
        );
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveRel(id));
        }
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
            id.clone(),
            Rel::Defined {
                not_typ: Box::new(not_typ),
                input_hint,
                rule_groups: vec![],
                else_group: None,
            },
        );
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveRel(id));
        }
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
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::PopRuleGroup(relid.clone()));
        }
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
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::ClearElseGroup(relid.clone()));
        }
        Ok(())
    }

    pub(super) fn find_table_func_opt(&self, id: &Id) -> Option<TableFuncState<'_>> {
        match self.fenv.get(id)? {
            Func::Table {
                params,
                typ_ret,
                table_rows,
            } => Some((params, typ_ret, table_rows)),
            _ => None,
        }
    }

    pub(super) fn find_table_func(&self, id: &Id) -> Result<TableFuncState<'_>, ElabError> {
        self.find_table_func_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::TableFunction, &id.node, id.span.clone())
        })
    }

    pub(super) fn find_defined_func_opt(&self, id: &Id) -> Option<DefinedFuncState<'_>> {
        match self.fenv.get(id)? {
            Func::Defined {
                tparams,
                params,
                typ_ret,
                clauses,
                else_clause,
            } => Some((tparams, params, typ_ret, clauses, else_clause.as_deref())),
            _ => None,
        }
    }

    pub(super) fn find_defined_func(&self, id: &Id) -> Result<DefinedFuncState<'_>, ElabError> {
        self.find_defined_func_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::DefinedFunction, &id.node, id.span.clone())
        })
    }

    pub(super) fn find_func_signature_opt(&self, id: &Id) -> Option<FuncSignature<'_>> {
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

    pub(super) fn find_func_signature(&self, id: &Id) -> Result<FuncSignature<'_>, ElabError> {
        self.find_func_signature_opt(id)
            .ok_or_else(|| ElabError::undefined(EntityKind::Function, &id.node, id.span.clone()))
    }

    pub(super) fn bound_func(&self, id: &Id) -> bool {
        self.find_func_signature_opt(id).is_some()
    }

    pub(super) fn add_extern_func(
        &mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<(), ElabError> {
        self.ensure_func_unbound(&id)?;
        self.fenv.insert(
            id.clone(),
            Func::Extern {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
            },
        );
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveFunc(id));
        }
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
            id.clone(),
            Func::Builtin {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
            },
        );
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveFunc(id));
        }
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
            id.clone(),
            Func::Table {
                params,
                typ_ret: Box::new(typ_ret),
                table_rows: vec![],
            },
        );
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveFunc(id));
        }
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
            id.clone(),
            Func::Defined {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
                clauses: vec![],
                else_clause: None,
            },
        );
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::RemoveFunc(id));
        }
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
        let Some((_, _, rows_found)) = self.find_table_func_opt(id) else {
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
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::ClearTableRows(id.clone()));
        }
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
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::PopClause(id.clone()));
        }
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
        if !self.checkpoints.is_empty() {
            self.undo.push(Undo::ClearElseClause(id.clone()));
        }
        Ok(())
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
