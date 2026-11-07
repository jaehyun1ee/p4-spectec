//! Encoding AL specifications and values into the mini p4st-in-p4st syntax
//!
//! `Encoder::spec` turns the AL of metasq-example/p4 into a `spec` meta-meta-value,
//! `Encoder::val` turns a meta-value into a `val`,
//! and `Encoder::unval` reads a `val` back as the meta-value it encodes.
//! Only the AL forms that the mini P4 specification produces are encoded;
//! any other form is reported instead of approximated.
//!
//! An iterated variable `x*{x <- x*}` encodes as `EVAR "x" [LIST]`,
//! the variable `x` under one more iteration.

use p4spectec::lang::{
    al::ast as al,
    common::{
        Iter,
        prim::{bool::CmpOp as BoolCmpOp, num::Number},
    },
    data::{
        typ::TypKind,
        value::tree::{Value, ValueKind},
    },
    el::ast::CmpOp,
    hints::input,
    il::ast::{ArgKind, DefTypKind, ListPattern, OptPattern, Pattern, TypDef},
};

use crate::value::{self as v, Error, Registry, ctor_of, typ_var};

/// Builds meta-meta-values of the mini p4st-in-p4st specification.
pub struct Encoder<'reg> {
    /// The cases of the mini p4st-in-p4st specification.
    meta: &'reg Registry,
}

/// A type name of the mini p4st-in-p4st syntax.
fn ty(name: &str) -> TypKind {
    typ_var(name)
}

impl<'reg> Encoder<'reg> {
    pub fn new(meta: &'reg Registry) -> Self {
        Self { meta }
    }

    fn case(&self, ctor: &str, args: Vec<Value>) -> Result<Value, Error> {
        self.meta.case(ctor, args)
    }

    // == Values

    /// Encodes a meta-value as a `val`.
    pub fn val(&self, value: Value) -> Result<Value, Error> {
        let note = value.note;
        match value.node {
            ValueKind::Bool(b) => self.case("VBOOL %", vec![v::bool(b)]),
            ValueKind::Num(Number::Nat(n)) => {
                let n = u64::try_from(n.as_bigint()).map_err(|error| error.to_string())?;
                self.case("VNAT %", vec![v::nat(n)])
            }
            ValueKind::Text(t) => self.case("VTEXT %", vec![v::text(&t)]),
            ValueKind::Case(value_case) => {
                let (mixop, args) = value_case.into_parts();
                let vals = self.vals(args)?;
                self.case("VCASE % %", vec![v::text(&ctor_of(&mixop)), vals])
            }
            ValueKind::Tuple(values) => self.case("VTUP %", vec![self.vals(values)?]),
            ValueKind::List(values) => self.case("VLIST %", vec![self.vals(values)?]),
            ValueKind::Opt(value) => {
                let value = value.map(|value| self.val(*value)).transpose()?;
                self.case("VOPT %", vec![v::opt(ty("val"), value)])
            }
            ValueKind::Extern(json) => {
                // Keep the extern type so that the value decodes as itself
                let typ_name = match &note {
                    TypKind::Var(id, _) => id.node.to_string(),
                    _ => return Err("an extern value has a named type".to_owned()),
                };
                let json = serde_json::json!({ "typ": typ_name, "json": *json });
                self.case("VEXT %", vec![v::external(ty("ext"), json)])
            }
            _ => Err(format!("mini p4st has no val for {note:?}")),
        }
    }

    fn vals(&self, values: Vec<Value>) -> Result<Value, Error> {
        let vals = values
            .into_iter()
            .map(|value| self.val(value))
            .collect::<Result<_, _>>()?;
        Ok(v::list(ty("val"), vals))
    }

    /// Decodes a `val` into the meta-value it encodes; `object` names its cases.
    pub fn unval(object: &Registry, val: Value) -> Result<Value, Error> {
        let (ctor, mut args) = v::uncase(val)?;
        let arg = args.pop().ok_or("a val has an argument")?;
        let unvals = |list: Value| -> Result<Vec<Value>, Error> {
            match list.node {
                ValueKind::List(vals) => vals
                    .into_iter()
                    .map(|val| Self::unval(object, val))
                    .collect(),
                _ => Err("expected a list of vals".to_owned()),
            }
        };
        match ctor.as_str() {
            "VBOOL %" | "VNAT %" | "VTEXT %" => Ok(arg),
            "VCASE % %" => {
                let ValueKind::Text(ctor) = args.pop().ok_or("VCASE has a ctor")?.node else {
                    return Err("a ctor is a text".to_owned());
                };
                object.case(&ctor, unvals(arg)?)
            }
            "VTUP %" => Ok(v::tuple(unvals(arg)?)),
            "VLIST %" => {
                let values = unvals(arg)?;
                // An empty list keeps no element type; name it after the meta-meta-value
                let elem = values.first().map_or(ty("val"), |value| value.note.clone());
                Ok(v::list(elem, values))
            }
            "VOPT %" => match arg.node {
                ValueKind::Opt(val) => {
                    let value = val.map(|val| Self::unval(object, *val)).transpose()?;
                    let elem = value.as_ref().map_or(ty("val"), |value| value.note.clone());
                    Ok(v::opt(elem, value))
                }
                _ => Err("VOPT holds an option".to_owned()),
            },
            "VEXT %" => match arg.node {
                ValueKind::Extern(json) => {
                    let typ_name = json["typ"].as_str().ok_or("VEXT names its type")?;
                    Ok(v::external(typ_var(typ_name), json["json"].clone()))
                }
                _ => Err("VEXT holds an extern value".to_owned()),
            },
            _ => Err(format!("unknown val `{ctor}`")),
        }
    }

    // == Expressions

    fn exps<'a>(&self, exps: impl IntoIterator<Item = &'a al::Exp>) -> Result<Value, Error> {
        let exps = exps
            .into_iter()
            .map(|exp| self.exp(exp))
            .collect::<Result<_, _>>()?;
        Ok(v::list(ty("exp"), exps))
    }

    fn evar(&self, id: &str, iters: &[Iter]) -> Result<Value, Error> {
        let its = iters
            .iter()
            .map(|iter| match iter {
                Iter::Opt => self.case("OPT", vec![]),
                Iter::List => self.case("LIST", vec![]),
            })
            .collect::<Result<_, _>>()?;
        self.case("EVAR % %", vec![v::text(id), v::list(ty("it"), its)])
    }

    /// Encodes an AL expression as an `exp`.
    pub fn exp(&self, exp: &al::Exp) -> Result<Value, Error> {
        use al::ExpKind as E;
        match &exp.node {
            E::Bool(b) => self.case("EBOOL %", vec![v::bool(*b)]),
            E::Num(Number::Nat(n)) => {
                let n = u64::try_from(n.as_bigint()).map_err(|error| error.to_string())?;
                self.case("ENAT %", vec![v::nat(n)])
            }
            E::Text(t) => self.case("ETEXT %", vec![v::text(t)]),
            E::Id(id) => self.evar(&id.node, &[]),
            E::Cmp(CmpOp::Bool(op), _, exp_l, exp_r) => {
                let ctor = match op {
                    BoolCmpOp::Eq => "ECMP % %",
                    BoolCmpOp::Ne => "ENE % %",
                };
                self.case(ctor, vec![self.exp(exp_l)?, self.exp(exp_r)?])
            }
            E::Match(exp, pattern) => {
                let pat = match pattern {
                    Pattern::Case(mixop) => self.case("PCASE %", vec![v::text(&ctor_of(mixop))])?,
                    Pattern::List(ListPattern::Cons) => self.case("PCONS", vec![])?,
                    Pattern::List(ListPattern::Nil) => self.case("PNIL", vec![])?,
                    Pattern::Opt(OptPattern::Some) => self.case("PSOME", vec![])?,
                    Pattern::Opt(OptPattern::None) => self.case("PNONE", vec![])?,
                    Pattern::List(ListPattern::Fixed(_)) => {
                        return Err("mini p4st has no fixed-length list pattern".to_owned());
                    }
                };
                self.case("EMATCH % %", vec![self.exp(exp)?, pat])
            }
            E::Tuple(exps) => self.case("ETUP %", vec![self.exps(exps)?]),
            E::Case(not_exp) => {
                let ctor = v::text(&ctor_of(not_exp.mixop()));
                self.case("ECASE % %", vec![ctor, self.exps(not_exp.args())?])
            }
            E::Opt(exp) => {
                let exp = exp.as_ref().map(|exp| self.exp(exp)).transpose()?;
                self.case("EOPT %", vec![v::opt(ty("exp"), exp)])
            }
            E::List(exps) => self.case("ELIST %", vec![self.exps(exps)?]),
            E::Cons(exp_h, exp_t) => {
                self.case("ECONS % %", vec![self.exp(exp_h)?, self.exp(exp_t)?])
            }
            E::Call(id, targs, args) if targs.is_empty() => {
                let exps = args
                    .iter()
                    .map(|arg| match &arg.node {
                        ArgKind::Exp(exp) => self.exp(exp),
                        ArgKind::Def(_) => Err("mini p4st has no function arguments".to_owned()),
                    })
                    .collect::<Result<_, _>>()?;
                self.case("ECALL % %", vec![v::text(&id.node), v::list(ty("exp"), exps)])
            }
            // An iterated variable `x*{x <- x*}` is `x` under one more iteration
            E::Iter(body, exp_iter) => match (&body.node, exp_iter.vars.as_slice()) {
                (E::Id(id), [var]) if var.id.node == id.node => {
                    let mut iters = var.iters.clone();
                    iters.push(exp_iter.iter);
                    self.evar(&id.node, &iters)
                }
                _ => Err("mini p4st iterates only variables".to_owned()),
            },
            _ => Err(format!(
                "mini p4st has no exp for {}",
                p4spectec::lang::traits::print::Print::to_string(exp)
            )),
        }
    }

    // == Premises

    fn prems<'a>(&self, prems: impl IntoIterator<Item = &'a al::Prem>) -> Result<Value, Error> {
        let prems = prems
            .into_iter()
            .map(|prem| self.prem(prem))
            .collect::<Result<_, _>>()?;
        Ok(v::list(ty("prem"), prems))
    }

    fn prem(&self, prem: &al::Prem) -> Result<Value, Error> {
        match &prem.node {
            al::PremKind::Rule(rule_prem) => {
                let exps = rule_prem.not_exp.args().iter().collect::<Vec<_>>();
                let (exps_input, exps_output) =
                    input::split(&rule_prem.input_hint, exps).map_err(|error| error.to_string())?;
                self.case(
                    "REL % % %",
                    vec![
                        v::text(&rule_prem.id.node),
                        self.exps(exps_input)?,
                        self.exps(exps_output)?,
                    ],
                )
            }
            al::PremKind::If(if_prem) => self.case("IF %", vec![self.exp(&if_prem.exp)?]),
            al::PremKind::Let(let_prem) => {
                self.case("LET % %", vec![self.exp(&let_prem.exp_l)?, self.exp(&let_prem.exp_r)?])
            }
            _ => Err("mini p4st has only IF, LET, and REL premises".to_owned()),
        }
    }

    // == Definitions

    /// A rule path of a group, with the group's match before its own premises.
    fn rul(&self, rule_match: &al::RuleMatch, rule_path: &al::RulePath) -> Result<Value, Error> {
        let prems = rule_match.prems.iter().chain(&rule_path.prems);
        self.case(
            "% % % %",
            vec![
                v::text(&rule_path.id.node),
                self.exps(&rule_match.exps_input)?,
                self.exps(&rule_path.exps_output)?,
                self.prems(prems)?,
            ],
        )
    }

    fn clause(&self, clause: &al::ClauseKind) -> Result<Value, Error> {
        let args = clause
            .args
            .iter()
            .map(|arg| match &arg.node {
                ArgKind::Exp(exp) => self.exp(exp),
                ArgKind::Def(_) => Err("mini p4st has no function parameters".to_owned()),
            })
            .collect::<Result<_, _>>()?;
        self.case(
            "% % %",
            vec![v::list(ty("exp"), args), self.exp(&clause.exp)?, self.prems(&clause.prems)?],
        )
    }

    fn defn(&self, def: &al::Def) -> Result<Option<Value>, Error> {
        let defn = match &def.node {
            al::DefKind::Typ(TypDef::Extern(extern_typ)) => {
                self.case("EXTSYND %", vec![v::text(&extern_typ.id.node)])?
            }
            al::DefKind::Typ(TypDef::Defined(typ_def)) => {
                // A type definition keeps only the notations of its cases
                let ctors = match &typ_def.def_typ.node {
                    DefTypKind::Variant(typ_cases) => typ_cases
                        .iter()
                        .map(|typ_case| v::text(&ctor_of(typ_case.not_typ.node.mixop())))
                        .collect(),
                    _ => Vec::new(),
                };
                self.case("SYND % %", vec![v::text(&typ_def.id.node), v::list(ty("ctor"), ctors)])?
            }
            al::DefKind::Var(_) => return Ok(None),
            al::DefKind::Rel(al::RelDef::Extern(extern_rel)) => {
                self.case("EXTRELD %", vec![v::text(&extern_rel.id.node)])?
            }
            al::DefKind::Rel(al::RelDef::Defined(rel)) => {
                let mut ruls = Vec::new();
                for rule_group in &rel.rule_groups {
                    for rule_path in &rule_group.node.rule_paths {
                        ruls.push(self.rul(&rule_group.node.rule_match, rule_path)?);
                    }
                }
                let rul_owise = rel
                    .else_group
                    .as_ref()
                    .map(|else_group| {
                        self.rul(&else_group.node.rule_match, &else_group.node.rule_path)
                    })
                    .transpose()?;
                self.case(
                    "RELD % % %",
                    vec![
                        v::text(&rel.id.node),
                        v::list(ty("rul"), ruls),
                        v::opt(ty("rul"), rul_owise),
                    ],
                )?
            }
            al::DefKind::MetaFunc(al::MetaFuncDef::Defined(func)) => {
                if !func.tparams.is_empty() {
                    return Err("mini p4st has no type parameters".to_owned());
                }
                let clauses = func
                    .clauses
                    .iter()
                    .map(|clause| self.clause(&clause.node))
                    .collect::<Result<_, _>>()?;
                let clause_owise = func
                    .else_clause
                    .as_ref()
                    .map(|else_clause| self.clause(&else_clause.node.clause))
                    .transpose()?;
                self.case(
                    "DECD % % %",
                    vec![
                        v::text(&func.id.node),
                        v::list(ty("clause"), clauses),
                        v::opt(ty("clause"), clause_owise),
                    ],
                )?
            }
            al::DefKind::MetaFunc(_) => {
                return Err("mini p4st has no extern, builtin, or table functions".to_owned());
            }
        };
        Ok(Some(defn))
    }

    /// Encodes an AL specification as a `spec`.
    pub fn spec(&self, spec: &al::Spec) -> Result<Value, Error> {
        let mut defns = Vec::new();
        for def in spec {
            if let Some(defn) = self.defn(def)? {
                defns.push(defn);
            }
        }
        Ok(v::list(ty("defn"), defns))
    }
}
