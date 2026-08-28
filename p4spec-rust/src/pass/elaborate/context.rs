//! Persistent elaboration bindings and operation-local fresh state

use crate::{
    lang::{
        common::{
            ds::set::IdSet,
            source::{Span, Spanned},
            Id,
        },
        il::ast,
    },
    runtime::{
        sta::{FEnv, MEnv, REnv, VEnv},
        types::{typ, TDEnv, TypeDef},
    },
};

use super::{ElabError, EntityKind};

/// Bindings and fresh state threaded through one elaboration operation
#[derive(Clone, Debug)]
pub(super) struct Context {
    pub(super) frees: IdSet,
    pub(super) venv: VEnv,
    pub(super) tdenv: TDEnv,
    pub(super) menv: MEnv,
    pub(super) renv: REnv,
    pub(super) fenv: FEnv,
    next_fresh: u64,
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
            let id = Spanned::new(name.to_owned(), Span::default());
            menv.insert(id, typ);
        }
        Self {
            frees: IdSet::new(),
            venv: VEnv::new(),
            tdenv: TDEnv::new(),
            menv,
            renv: REnv::new(),
            fenv: FEnv::new(),
            next_fresh: 0,
        }
    }

    pub(super) fn fresh_index(&mut self) -> u64 {
        let fresh = self.next_fresh;
        self.next_fresh += 1;
        fresh
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

    pub(super) fn find_metavar(&self, id: &Id) -> Result<&ast::Typ, ElabError> {
        self.find_metavar_opt(id).ok_or_else(|| {
            ElabError::undefined(EntityKind::MetaVariable, &id.node, id.span.clone())
        })
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
        self.menv.insert(id, typ);
        Ok(self)
    }

    pub(super) fn add_typdef(mut self, id: Id, typdef: TypeDef) -> Result<Self, ElabError> {
        if self.bound_typdef(&id) {
            return Err(ElabError::duplicate(EntityKind::Type, &id.node, id.span));
        }
        self.tdenv.insert(id, typdef);
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
        self.tdenv.insert(id.clone(), typdef);
        Ok(self)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        lang::{
            common::source::{Position, Span, Spanned},
            il::ast::TypKind,
        },
        pass::elaborate::{ElabErrorKind, EntityKind},
        runtime::types::TypeDef,
    };

    use super::Context;

    fn span(label: &str) -> Span {
        Span::new(Position::new(label, 1, 2), Position::new(label, 1, 5))
    }

    fn id(name: &str, span: Span) -> Spanned<String> {
        Spanned::new(name.to_owned(), span)
    }

    #[test]
    fn initial_context_binds_the_builtin_metavariable_types() {
        let ctx = Context::new();

        assert_eq!(
            ctx.find_metavar(&id("bool", Span::default()))
                .expect("bool")
                .node,
            TypKind::Bool
        );
        assert_eq!(
            ctx.find_metavar(&id("nat", Span::default()))
                .expect("nat")
                .node,
            TypKind::Num(crate::lang::xl::num::Typ::Nat)
        );
        assert_eq!(
            ctx.find_metavar(&id("int", Span::default()))
                .expect("int")
                .node,
            TypKind::Num(crate::lang::xl::num::Typ::Int)
        );
        assert_eq!(
            ctx.find_metavar(&id("text", Span::default()))
                .expect("text")
                .node,
            TypKind::Text
        );
    }

    #[test]
    fn duplicate_metavariable_reports_the_second_definition_span() {
        let first = id("item", span("first"));
        let second_span = span("second");
        let second = id("item", second_span.clone());
        let ctx = Context::new()
            .add_metavar(first, crate::runtime::types::typ::bool())
            .expect("first definition");

        let error = ctx
            .add_metavar(second, crate::runtime::types::typ::text())
            .unwrap_err();

        assert_eq!(
            error.kind,
            ElabErrorKind::Duplicate(EntityKind::MetaVariable)
        );
        assert_eq!(error.span, second_span);
    }

    #[test]
    fn missing_type_reports_the_query_span() {
        let query_span = span("query");
        let query = id("Missing", query_span.clone());

        let error = Context::new().find_typdef(&query).unwrap_err();

        assert_eq!(error.kind, ElabErrorKind::Undefined(EntityKind::Type));
        assert_eq!(error.span, query_span);
    }

    #[test]
    fn type_parameter_binds_both_type_definition_and_metavariable() {
        let tparam = id("T", span("parameter"));
        let ctx = Context::new()
            .add_tparam(tparam.clone())
            .expect("type parameter");

        assert_eq!(ctx.find_typdef(&tparam), Ok(&TypeDef::Parameter));
        assert!(matches!(
            &ctx.find_metavar(&tparam).expect("metavariable").node,
            TypKind::Var(id, targs) if id == &tparam && targs.is_empty()
        ));
    }

    #[test]
    fn fresh_state_is_scoped_to_each_context() {
        let mut ctx_l = Context::new();
        let mut ctx_r = Context::new();

        assert_eq!(ctx_l.fresh_index(), 0);
        assert_eq!(ctx_l.fresh_index(), 1);
        assert_eq!(ctx_r.fresh_index(), 0);
    }

    #[test]
    fn free_bindings_keep_the_first_source_location() {
        let first_span = span("first-free");
        let first = id("item", first_span.clone());
        let repeated = id("item", span("repeated-free"));
        let ctx = Context::new().add_free(first).add_free(repeated);

        assert_eq!(
            ctx.frees.get(&id("item", Span::default())).unwrap().span,
            first_span
        );
    }

    #[test]
    fn type_definition_updates_require_an_existing_binding() {
        let missing_span = span("missing-update");
        let missing = id("Missing", missing_span.clone());
        let error = Context::new()
            .update_typdef(&missing, TypeDef::Extern)
            .unwrap_err();
        assert_eq!(error.kind, ElabErrorKind::Undefined(EntityKind::Type));
        assert_eq!(error.span, missing_span);

        let defined = id("Known", span("known"));
        let ctx = Context::new()
            .add_typdef(defined.clone(), TypeDef::Defining(vec![]))
            .expect("declare type")
            .update_typdef(&defined, TypeDef::Extern)
            .expect("complete type");
        assert_eq!(ctx.find_typdef(&defined), Ok(&TypeDef::Extern));
    }
}
