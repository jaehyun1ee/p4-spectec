//! Definition and hint state for prose conversion

use std::collections::BTreeMap;

use crate::lang::{
    common::{notation::mixop::Mixop, source::Span},
    data::typ,
    hints::{alter, fields},
    il,
    pl::annot::Hints,
    sl::ast::{self as sl, Id},
};
use crate::runtime::envs::algo::MEnv;

use super::{ProseError, ProseErrorKind};

// == Context

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum HintKey {
    Case(String, Mixop),
    Func(String),
    Rel(String),
}

type HEnv = BTreeMap<HintKey, Hints>;

#[derive(Debug)]
pub(super) struct Context {
    id_namespace: Option<Id>,
    henv: HEnv,
    menv: MEnv,
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
        Self { id_namespace: None, henv: HEnv::new(), menv }
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
        self.henv.get(&HintKey::Func(id_func.node.clone()))
    }

    pub(super) fn hints_rel(&self, id_rel: &Id) -> Option<&Hints> {
        self.henv.get(&HintKey::Rel(id_rel.node.clone()))
    }

    pub(super) fn hints_case(&self, id_typ: &Id, mixop: &Mixop) -> Option<&Hints> {
        self.henv
            .get(&HintKey::Case(id_typ.node.clone(), mixop.clone()))
    }

    // - Metavariables

    pub(super) fn menv(&self) -> &MEnv {
        &self.menv
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
                }
                sl::DefKind::Typ(sl::TypDef::Defined(def_typ_sl)) => {
                    if def_typ_sl.tparams.is_empty() {
                        let typ = crate::phrase! {
                            node: il::ast::TypKind::Var(def_typ_sl.id.clone(), Vec::new()),
                            span: def_typ_sl.id.span.clone(),
                        };
                        ctx.add_metavar(def_typ_sl.id.clone(), typ)?;
                    }
                    if let il::ast::DefTypKind::Variant(cases) = &def_typ_sl.def_typ.node {
                        for (not_typ, _, hints_sl) in cases {
                            let key =
                                HintKey::Case(def_typ_sl.id.node.clone(), not_typ.node.to_mixop());
                            let hints = Self::load_hints(hints_sl)?;
                            ctx.henv.insert(key, hints);
                        }
                    }
                }
                sl::DefKind::Var(def_var_sl) => {
                    ctx.add_metavar(def_var_sl.id.clone(), def_var_sl.typ.clone())?;
                }
                sl::DefKind::Rel(sl::RelDef::Extern(def_rel_sl)) => {
                    let key = HintKey::Rel(def_rel_sl.id.node.clone());
                    let hints = Self::load_hints(&def_rel_sl.hints)?;
                    ctx.henv.insert(key, hints);
                }
                sl::DefKind::Rel(sl::RelDef::Defined(def_rel_sl)) => {
                    let key = HintKey::Rel(def_rel_sl.id.node.clone());
                    let hints = Self::load_hints(&def_rel_sl.hints)?;
                    ctx.henv.insert(key, hints);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Extern(def_func_sl)) => {
                    let key = HintKey::Func(def_func_sl.id.node.clone());
                    let hints = Self::load_hints(&def_func_sl.hints)?;
                    ctx.henv.insert(key, hints);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Builtin(def_func_sl)) => {
                    let key = HintKey::Func(def_func_sl.id.node.clone());
                    let hints = Self::load_hints(&def_func_sl.hints)?;
                    ctx.henv.insert(key, hints);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Table(def_func_sl)) => {
                    let key = HintKey::Func(def_func_sl.id.node.clone());
                    let hints = Self::load_hints(&def_func_sl.hints)?;
                    ctx.henv.insert(key, hints);
                }
                sl::DefKind::MetaFunc(sl::MetaFuncDef::Defined(def_func_sl)) => {
                    let key = HintKey::Func(def_func_sl.id.node.clone());
                    let hints = Self::load_hints(&def_func_sl.hints)?;
                    ctx.henv.insert(key, hints);
                }
            }
        }
        Ok(ctx)
    }
}
