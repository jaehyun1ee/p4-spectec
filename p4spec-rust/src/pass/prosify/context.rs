//! Definition and hint state for prose conversion

use crate::lang::{
    common::{notation::mixop::Mixop, source::Span},
    data::typ,
    hints::{alter, fields},
    il,
    pl::annot::Hints,
    sl::ast::{self as sl, Id},
};
use crate::runtime::envs::{algo::MEnv, prosify::HEnv};

use super::{ProseError, ProseErrorKind};

// == Context

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
        Self { id_namespace: None, henv: HEnv::default(), menv }
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
        self.henv.get_func(id_func)
    }

    pub(super) fn hints_rel(&self, id_rel: &Id) -> Option<&Hints> {
        self.henv.get_rel(id_rel)
    }

    pub(super) fn hints_case(&self, id_typ: &Id, mixop: &Mixop) -> Option<&Hints> {
        self.henv.get_case(id_typ, mixop)
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
        for sl::Hint { id: id_hint, exp: exp_hint } in hints_sl {
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

    fn load_def(&mut self, def_sl: &sl::Def) -> Result<(), ProseError> {
        match &def_sl.node {
            sl::DefKind::Typ(def_typ_sl) => self.load_typ_def(def_typ_sl),
            sl::DefKind::Var(def_var_sl) => self.load_var_def(def_var_sl),
            sl::DefKind::Rel(def_rel_sl) => self.load_rel_def(def_rel_sl),
            sl::DefKind::MetaFunc(def_func_sl) => self.load_func_def(def_func_sl),
        }
    }

    fn load_typ_def(&mut self, def_typ_sl: &sl::TypDef) -> Result<(), ProseError> {
        match def_typ_sl {
            sl::TypDef::Extern(def_typ_sl) => self.load_extern_typ_def(def_typ_sl),
            sl::TypDef::Defined(def_typ_sl) => self.load_defined_typ_def(def_typ_sl),
        }
    }

    fn load_extern_typ_def(&mut self, def_typ_sl: &sl::ExternTyp) -> Result<(), ProseError> {
        let typ = crate::phrase! {
            node: il::ast::TypKind::Var(def_typ_sl.id.clone(), Vec::new()),
            span: def_typ_sl.id.span.clone(),
        };
        self.add_metavar(def_typ_sl.id.clone(), typ)
    }

    fn load_defined_typ_def(&mut self, def_typ_sl: &sl::DefinedTyp) -> Result<(), ProseError> {
        if def_typ_sl.tparams.is_empty() {
            let typ = crate::phrase! {
                node: il::ast::TypKind::Var(def_typ_sl.id.clone(), Vec::new()),
                span: def_typ_sl.id.span.clone(),
            };
            self.add_metavar(def_typ_sl.id.clone(), typ)?;
        }
        let il::ast::DefTypKind::Variant(cases) = &def_typ_sl.def_typ.node else {
            return Ok(());
        };
        for il::ast::TypCase { not_typ, hints: hints_sl, .. } in cases {
            let hints = Self::load_hints(hints_sl)?;
            self.henv
                .insert_case(&def_typ_sl.id, &not_typ.node.to_mixop(), hints);
        }
        Ok(())
    }

    fn load_var_def(&mut self, def_var_sl: &sl::VarDef) -> Result<(), ProseError> {
        self.add_metavar(def_var_sl.id.clone(), def_var_sl.typ.clone())
    }

    fn load_rel_def(&mut self, def_rel_sl: &sl::RelDef) -> Result<(), ProseError> {
        let (id_rel, hints_sl) = match def_rel_sl {
            sl::RelDef::Extern(def_rel_sl) => (&def_rel_sl.id, &def_rel_sl.hints),
            sl::RelDef::Defined(def_rel_sl) => (&def_rel_sl.id, &def_rel_sl.hints),
        };
        let hints = Self::load_hints(hints_sl)?;
        self.henv.insert_rel(id_rel, hints);
        Ok(())
    }

    fn load_func_def(&mut self, def_func_sl: &sl::MetaFuncDef) -> Result<(), ProseError> {
        let (id_func, hints_sl) = match def_func_sl {
            sl::MetaFuncDef::Extern(def_func_sl) => (&def_func_sl.id, &def_func_sl.hints),
            sl::MetaFuncDef::Builtin(def_func_sl) => (&def_func_sl.id, &def_func_sl.hints),
            sl::MetaFuncDef::Table(def_func_sl) => (&def_func_sl.id, &def_func_sl.hints),
            sl::MetaFuncDef::Defined(def_func_sl) => (&def_func_sl.id, &def_func_sl.hints),
        };
        let hints = Self::load_hints(hints_sl)?;
        self.henv.insert_func(id_func, hints);
        Ok(())
    }

    pub(super) fn load(spec_sl: &sl::Spec) -> Result<Self, ProseError> {
        let mut ctx = Self::init();
        for def_sl in spec_sl {
            ctx.load_def(def_sl)?;
        }
        Ok(ctx)
    }
}
