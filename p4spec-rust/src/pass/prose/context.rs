//! Definition and hint state for prose conversion

use crate::lang::{
    common::{
        ds::{map::IdMap, set::IdSet},
        notation::mixop::Mixop,
        source::Span,
    },
    data::typ,
    hints::{alter, fields},
    il,
    pl::annot::Hints,
    sl::ast::{self as sl, Id},
    traits::eq::SyntaxEq,
};
use crate::runtime::envs::algo::MEnv;

use super::{ProseError, ProseErrorKind};

// == Context

#[derive(Debug)]
pub(super) struct Context {
    id_namespace: Option<Id>,
    hints_func: IdMap<Hints>,
    hints_rel: IdMap<Hints>,
    hints_case: Vec<(String, Mixop, Hints)>,
    menv: MEnv,
    ids_typ: IdSet,
}

impl Context {
    // - Constructor

    fn init() -> Self {
        let mut menv = MEnv::new();
        for (text_name, typ) in [
            ("bool", typ::make::bool()),
            ("nat", typ::make::nat()),
            ("int", typ::make::int()),
            ("text", typ::make::text()),
        ] {
            let id = crate::phrase! { node: text_name.to_owned(), span: Span::default() };
            menv.insert(id, typ);
        }
        Self {
            id_namespace: None,
            hints_func: IdMap::new(),
            hints_rel: IdMap::new(),
            hints_case: Vec::new(),
            menv,
            ids_typ: IdSet::new(),
        }
    }

    // - Namespace

    pub(super) fn set_namespace(&mut self, id_namespace: Id) {
        self.id_namespace = Some(id_namespace);
    }

    pub(super) fn namespace(&self) -> &Id {
        self.id_namespace
            .as_ref()
            .expect("relation conversion establishes its namespace")
    }

    // - Hint lookup

    pub(super) fn hints_func(&self, id_func: &Id) -> Option<&Hints> {
        self.hints_func.get(id_func)
    }

    pub(super) fn hints_rel(&self, id_rel: &Id) -> Option<&Hints> {
        self.hints_rel.get(id_rel)
    }

    pub(super) fn hints_case(&self, id_typ: &Id, mixop: &Mixop) -> Option<&Hints> {
        self.hints_case
            .iter()
            .find(|(text_typ, mixop_case, _)| {
                text_typ == &id_typ.node && mixop_case.syntax_eq(mixop)
            })
            .map(|(_, _, hints)| hints)
    }

    // - Metavariables

    pub(super) fn menv(&self) -> &MEnv {
        &self.menv
    }

    // - Type parameters

    pub(super) fn validate_tparams(&self, tparams: &[il::ast::TParam]) -> Result<(), ProseError> {
        let mut ids_typ_local = IdSet::new();
        for id_tparam in tparams {
            if self.ids_typ.contains(id_tparam) || ids_typ_local.contains(id_tparam) {
                return Err(ProseError::new(ProseErrorKind::DuplicateType, id_tparam.span.clone()));
            }
            ids_typ_local.insert(id_tparam.clone());
        }
        Ok(())
    }

    // - Adders

    fn add_metavar(&mut self, id_metavar: Id, typ: il::ast::Typ) -> Result<(), ProseError> {
        if self.menv.contains_key(&id_metavar) {
            return Err(ProseError::new(
                ProseErrorKind::DuplicateMetavariable,
                id_metavar.span.clone(),
            ));
        }
        self.menv.insert(id_metavar, typ);
        Ok(())
    }

    fn add_type(&mut self, id_typ: Id) -> Result<(), ProseError> {
        if self.ids_typ.contains(&id_typ) {
            return Err(ProseError::new(ProseErrorKind::DuplicateType, id_typ.span.clone()));
        }
        self.ids_typ.insert(id_typ);
        Ok(())
    }

    // - Hint loading

    fn load_hints(hints_sl: &[sl::Hint]) -> Result<Hints, ProseError> {
        let mut hints = Hints::default();
        for (id_hint, exp_hint) in hints_sl {
            let text_hint = id_hint.node.as_str();
            match text_hint {
                "prose" | "prose_in" | "prose_out" | "prose_true" | "prose_false" => {
                    let hint = alter::init(exp_hint).ok_or_else(|| {
                        ProseError::new(
                            ProseErrorKind::InvalidHintExpression(text_hint.to_owned()),
                            exp_hint.span.clone(),
                        )
                    })?;
                    match text_hint {
                        "prose" => hints.prose = Some(hint),
                        "prose_in" => hints.prose_in = Some(hint),
                        "prose_out" => hints.prose_out = Some(hint),
                        "prose_true" => hints.prose_true = Some(hint),
                        "prose_false" => hints.prose_false = Some(hint),
                        _ => unreachable!(),
                    }
                }
                "prose_fields" => {
                    hints.prose_fields = Some(fields::init(exp_hint).ok_or_else(|| {
                        ProseError::new(
                            ProseErrorKind::InvalidHintExpression(text_hint.to_owned()),
                            exp_hint.span.clone(),
                        )
                    })?);
                }
                _ => {}
            }
        }
        Ok(hints)
    }

    // - Definition loading

    pub(super) fn load(spec_sl: &sl::Spec) -> Result<Self, ProseError> {
        let mut ctx = Self::init();
        for def_sl in spec_sl {
            match &def_sl.node {
                sl::DefKind::Typ(sl::TypDef::Extern(def_typ_sl)) => {
                    let typ = crate::phrase! {
                        node: il::ast::TypKind::Var(def_typ_sl.id.clone(), Vec::new()),
                        span: def_typ_sl.id.span.clone(),
                    };
                    ctx.add_metavar(def_typ_sl.id.clone(), typ)?;
                    ctx.add_type(def_typ_sl.id.clone())?;
                }
                sl::DefKind::Typ(sl::TypDef::Defined(def_typ_sl)) => {
                    if def_typ_sl.tparams.is_empty() {
                        let typ = crate::phrase! {
                            node: il::ast::TypKind::Var(def_typ_sl.id.clone(), Vec::new()),
                            span: def_typ_sl.id.span.clone(),
                        };
                        ctx.add_metavar(def_typ_sl.id.clone(), typ)?;
                    }
                    ctx.add_type(def_typ_sl.id.clone())?;
                    if let il::ast::DefTypKind::Variant(cases) = &def_typ_sl.def_typ.node {
                        for (not_typ, _, hints_sl) in cases {
                            ctx.hints_case.push((
                                def_typ_sl.id.node.clone(),
                                not_typ.node.to_mixop(),
                                Self::load_hints(hints_sl)?,
                            ));
                        }
                    }
                }
                sl::DefKind::Var(def_var_sl) => {
                    ctx.add_metavar(def_var_sl.id.clone(), def_var_sl.typ.clone())?;
                }
                sl::DefKind::Rel(sl::RelDef::Extern(def_rel_sl)) => {
                    ctx.hints_rel
                        .insert(def_rel_sl.id.clone(), Self::load_hints(&def_rel_sl.hints)?);
                }
                sl::DefKind::Rel(sl::RelDef::Defined(def_rel_sl)) => {
                    ctx.hints_rel
                        .insert(def_rel_sl.id.clone(), Self::load_hints(&def_rel_sl.hints)?);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Extern(def_func_sl)) => {
                    ctx.hints_func
                        .insert(def_func_sl.id.clone(), Self::load_hints(&def_func_sl.hints)?);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Builtin(def_func_sl)) => {
                    ctx.hints_func
                        .insert(def_func_sl.id.clone(), Self::load_hints(&def_func_sl.hints)?);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Table(def_func_sl)) => {
                    ctx.hints_func
                        .insert(def_func_sl.id.clone(), Self::load_hints(&def_func_sl.hints)?);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(def_func_sl)) => {
                    ctx.hints_func
                        .insert(def_func_sl.id.clone(), Self::load_hints(&def_func_sl.hints)?);
                }
            }
        }
        Ok(ctx)
    }
}
