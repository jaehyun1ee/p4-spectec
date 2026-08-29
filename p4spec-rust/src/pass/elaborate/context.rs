//! Persistent elaboration bindings and operation-local fresh state

use std::rc::Rc;

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
#[derive(Clone, Debug)]
pub(super) struct Context {
    pub(super) frees: IdSet,
    pub(super) tdenv: Rc<TDEnv>,
    pub(super) menv: Rc<MEnv>,
    pub(super) renv: Rc<REnv>,
    pub(super) fenv: Rc<FEnv>,
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
            tdenv: Rc::new(TDEnv::new()),
            menv: Rc::new(menv),
            renv: Rc::new(REnv::new()),
            fenv: Rc::new(FEnv::new()),
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

    pub(super) fn add_free(mut self, id: Id) -> Self {
        self.frees.insert(id);
        self
    }

    pub(super) fn add_frees(mut self, ids: IdSet) -> Self {
        self.frees.extend(ids.iter().cloned());
        self
    }

    pub(super) fn add_metavar(mut self, id: Id, typ: ast::Typ) -> Result<Self, ElabError> {
        if self.bound_metavar(&id) {
            return Err(ElabError::duplicate(
                EntityKind::MetaVariable,
                &id.node,
                id.span,
            ));
        }
        Rc::make_mut(&mut self.menv).insert(id, typ);
        Ok(self)
    }

    pub(super) fn add_typdef(mut self, id: Id, typdef: TypeDef) -> Result<Self, ElabError> {
        if self.bound_typdef(&id) {
            return Err(ElabError::duplicate(EntityKind::Type, &id.node, id.span));
        }
        Rc::make_mut(&mut self.tdenv).insert(id, typdef);
        Ok(self)
    }

    pub(super) fn add_tparam(self, tparam: ast::TParam) -> Result<Self, ElabError> {
        let typ = typ::var(tparam.clone(), vec![]);
        let ctx = self.add_typdef(tparam.clone(), TypeDef::Parameter)?;
        ctx.add_metavar(tparam, typ)
    }

    pub(super) fn add_tparams(mut self, tparams: &[ast::TParam]) -> Result<Self, ElabError> {
        for tparam in tparams {
            self = self.add_tparam(tparam.clone())?;
        }
        Ok(self)
    }

    pub(super) fn update_typdef(mut self, id: &Id, typdef: TypeDef) -> Result<Self, ElabError> {
        if !self.bound_typdef(id) {
            return Err(ElabError::undefined(
                EntityKind::Type,
                &id.node,
                id.span.clone(),
            ));
        }
        Rc::make_mut(&mut self.tdenv).insert(id.clone(), typdef);
        Ok(self)
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
        mut self,
        id: Id,
        not_typ: ast::NotTyp,
        input_hint: InputHint,
    ) -> Result<Self, ElabError> {
        if self.bound_rel(&id) {
            return Err(ElabError::duplicate(
                EntityKind::Relation,
                &id.node,
                id.span,
            ));
        }
        Rc::make_mut(&mut self.renv).insert(
            id,
            Rel::Extern {
                not_typ: Box::new(not_typ),
                input_hint,
            },
        );
        Ok(self)
    }

    pub(super) fn add_defined_rel(
        mut self,
        id: Id,
        not_typ: ast::NotTyp,
        input_hint: InputHint,
    ) -> Result<Self, ElabError> {
        if self.bound_rel(&id) {
            return Err(ElabError::duplicate(
                EntityKind::Relation,
                &id.node,
                id.span,
            ));
        }
        Rc::make_mut(&mut self.renv).insert(
            id,
            Rel::Defined {
                not_typ: Box::new(not_typ),
                input_hint,
                rule_groups: vec![],
                else_group: None,
            },
        );
        Ok(self)
    }

    pub(super) fn add_defined_rule_group(
        mut self,
        relid: &Id,
        rule_group: ast::RuleGroup,
    ) -> Result<Self, ElabError> {
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
        let Rel::Defined { rule_groups, .. } = Rc::make_mut(&mut self.renv)
            .get_mut(relid)
            .expect("defined relation")
        else {
            unreachable!("checked defined relation")
        };
        rule_groups.push(rule_group);
        Ok(self)
    }

    pub(super) fn add_defined_else_group(
        mut self,
        relid: &Id,
        else_group: ast::ElseGroup,
    ) -> Result<Self, ElabError> {
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
        } = Rc::make_mut(&mut self.renv)
            .get_mut(relid)
            .expect("defined relation")
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
        Ok(self)
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
        mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<Self, ElabError> {
        self.ensure_func_unbound(&id)?;
        Rc::make_mut(&mut self.fenv).insert(
            id,
            Func::Extern {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
            },
        );
        Ok(self)
    }

    pub(super) fn add_builtin_func(
        mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<Self, ElabError> {
        self.ensure_func_unbound(&id)?;
        Rc::make_mut(&mut self.fenv).insert(
            id,
            Func::Builtin {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
            },
        );
        Ok(self)
    }

    pub(super) fn add_table_func(
        mut self,
        id: Id,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<Self, ElabError> {
        self.ensure_func_unbound(&id)?;
        Rc::make_mut(&mut self.fenv).insert(
            id,
            Func::Table {
                params,
                typ_ret: Box::new(typ_ret),
                table_rows: vec![],
            },
        );
        Ok(self)
    }

    pub(super) fn add_defined_func(
        mut self,
        id: Id,
        tparams: Vec<ast::TParam>,
        params: Vec<ast::Param>,
        typ_ret: ast::Typ,
    ) -> Result<Self, ElabError> {
        self.ensure_func_unbound(&id)?;
        Rc::make_mut(&mut self.fenv).insert(
            id,
            Func::Defined {
                tparams,
                params,
                typ_ret: Box::new(typ_ret),
                clauses: vec![],
                else_clause: None,
            },
        );
        Ok(self)
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
        mut self,
        id: &Id,
        table_rows: Vec<ast::TableRow>,
    ) -> Result<Self, ElabError> {
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
        } = Rc::make_mut(&mut self.fenv)
            .get_mut(id)
            .expect("table function")
        else {
            unreachable!("checked table function")
        };
        *stored = table_rows;
        Ok(self)
    }

    pub(super) fn add_defined_func_clause(
        mut self,
        id: &Id,
        clause: ast::Clause,
    ) -> Result<Self, ElabError> {
        if self.find_defined_func_opt(id).is_none() {
            return Err(ElabError::undefined(
                EntityKind::Function,
                &id.node,
                clause.span,
            ));
        }
        let Func::Defined { clauses, .. } = Rc::make_mut(&mut self.fenv)
            .get_mut(id)
            .expect("defined function")
        else {
            unreachable!("checked defined function")
        };
        clauses.push(clause);
        Ok(self)
    }

    pub(super) fn add_defined_func_else_clause(
        mut self,
        id: &Id,
        else_clause: ast::ElseClause,
    ) -> Result<Self, ElabError> {
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
        } = Rc::make_mut(&mut self.fenv)
            .get_mut(id)
            .expect("defined function")
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
        Ok(self)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
