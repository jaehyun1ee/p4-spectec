//! Definition and hint state for prose conversion

use crate::lang::{
    common::{ds::map::IdMap, notation::mixop::Mixop, source::Span},
    data::typ,
    hints::{alter, fields},
    il,
    pl::annot::Hints,
    sl::ast::{self as sl, Id},
    traits::eq::SyntaxEq,
};
use crate::runtime::envs::algo::MEnv;

use super::{ProseError, ProseErrorKind};

#[derive(Debug)]
pub(super) struct Context {
    id_namespace: Option<Id>,
    hints_func: IdMap<Hints>,
    hints_rel: IdMap<Hints>,
    hints_case: Vec<(String, Mixop, Hints)>,
    metavars: MEnv,
    types: IdMap<()>,
}

impl Context {
    fn init() -> Self {
        let mut metavars = MEnv::new();
        for (name, typ) in [
            ("bool", typ::make::bool()),
            ("nat", typ::make::nat()),
            ("int", typ::make::int()),
            ("text", typ::make::text()),
        ] {
            let id = crate::phrase! { node: name.to_owned(), span: Span::default() };
            metavars.insert(id, typ);
        }
        Self {
            id_namespace: None,
            hints_func: IdMap::new(),
            hints_rel: IdMap::new(),
            hints_case: Vec::new(),
            metavars,
            types: IdMap::new(),
        }
    }

    pub(super) fn set_namespace(&mut self, id: Id) {
        self.id_namespace = Some(id);
    }

    pub(super) fn namespace(&self) -> &Id {
        self.id_namespace
            .as_ref()
            .expect("relation conversion establishes its namespace")
    }

    pub(super) fn func_hints(&self, id: &Id) -> Hints {
        self.hints_func.get(id).cloned().unwrap_or_default()
    }

    pub(super) fn rel_hints(&self, id: &Id) -> Hints {
        self.hints_rel.get(id).cloned().unwrap_or_default()
    }

    pub(super) fn case_hints(&self, id: &Id, mixop: &Mixop) -> Hints {
        self.hints_case
            .iter()
            .find(|(id_typ, mixop_case, _)| id_typ == &id.node && mixop_case.syntax_eq(mixop))
            .map(|(_, _, hints)| hints.clone())
            .unwrap_or_default()
    }

    pub(super) fn metavars(&self) -> &MEnv {
        &self.metavars
    }

    pub(super) fn validate_tparams(&self, tparams: &[il::ast::TParam]) -> Result<(), ProseError> {
        let mut types_local = IdMap::new();
        for id in tparams {
            if self.types.contains_key(id) || types_local.contains_key(id) {
                return Err(ProseError::new(ProseErrorKind::DuplicateType, id.span.clone()));
            }
            types_local.insert(id.clone(), ());
        }
        Ok(())
    }

    fn load_hints(hints_sl: &[sl::Hint]) -> Result<Hints, ProseError> {
        let mut hints = Hints::default();
        for (id_hint, exp_hint) in hints_sl {
            let hint_name = id_hint.node.as_str();
            match hint_name {
                "prose" | "prose_in" | "prose_out" | "prose_true" | "prose_false" => {
                    let hint = alter::init(exp_hint).ok_or_else(|| {
                        ProseError::new(
                            ProseErrorKind::InvalidHintExpression(hint_name.to_owned()),
                            exp_hint.span.clone(),
                        )
                    })?;
                    match hint_name {
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
                            ProseErrorKind::InvalidHintExpression(hint_name.to_owned()),
                            exp_hint.span.clone(),
                        )
                    })?);
                }
                _ => {}
            }
        }
        Ok(hints)
    }

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

    fn add_metavar(&mut self, id: Id, typ: il::ast::Typ) -> Result<(), ProseError> {
        if self.metavars.contains_key(&id) {
            return Err(ProseError::new(ProseErrorKind::DuplicateMetavariable, id.span.clone()));
        }
        self.metavars.insert(id, typ);
        Ok(())
    }

    fn add_type(&mut self, id: Id) -> Result<(), ProseError> {
        if self.types.contains_key(&id) {
            return Err(ProseError::new(ProseErrorKind::DuplicateType, id.span.clone()));
        }
        self.types.insert(id, ());
        Ok(())
    }
}
