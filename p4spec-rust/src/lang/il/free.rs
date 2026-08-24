use std::collections::BTreeSet;

use super::ast::*;

// Identifier set

pub type FreeVars = BTreeSet<IdKind>;
pub fn empty() -> FreeVars {
    FreeVars::new()
}
pub fn singleton(id: &Id) -> FreeVars {
    FreeVars::from([id.node.clone()])
}
fn add(a: FreeVars, b: FreeVars) -> FreeVars {
    a.into_iter().chain(b).collect()
}
fn many<TItem>(xs: &[TItem], f: impl Fn(&TItem) -> FreeVars) -> FreeVars {
    xs.iter().fold(FreeVars::new(), |a, x| add(a, f(x)))
}

// Collect free identifiers

// Expressions

pub fn free_exp(exp: &Exp) -> FreeVars {
    match &exp.kind {
        ExpKind::BoolE(_) | ExpKind::NumE(_) | ExpKind::TextE(_) => empty(),
        ExpKind::VarE(id) => singleton(id),
        ExpKind::UnE(_, _, x)
        | ExpKind::UpCastE(_, x)
        | ExpKind::DownCastE(_, x)
        | ExpKind::SubE(x, _, _)
        | ExpKind::MatchE(x, _)
        | ExpKind::LenE(x)
        | ExpKind::DotE(x, _)
        | ExpKind::IterE(x, _) => free_exp(x),
        ExpKind::BinE(_, _, a, b)
        | ExpKind::CmpE(_, _, a, b)
        | ExpKind::ConsE(a, b)
        | ExpKind::CatE(a, b)
        | ExpKind::MemE(a, b)
        | ExpKind::IdxE(a, b) => add(free_exp(a), free_exp(b)),
        ExpKind::TupleE(xs) | ExpKind::ListE(xs) => free_exps(xs),
        ExpKind::CaseE(m) => m
            .args()
            .into_iter()
            .fold(FreeVars::new(), |a, x| add(a, free_exp(x))),
        ExpKind::StrE(xs) => many(xs, |(_, x)| free_exp(x)),
        ExpKind::OptE(Some(x)) => free_exp(x),
        ExpKind::OptE(None) => empty(),
        ExpKind::SliceE(a, b, c) => add(free_exp(a), add(free_exp(b), free_exp(c))),
        ExpKind::UpdE(a, p, b) => add(free_exp(a), add(free_path(p), free_exp(b))),
        ExpKind::CallE(_, _, args) => free_args(args),
    }
}
pub fn free_exps(xs: &[Exp]) -> FreeVars {
    many(xs, free_exp)
}

// Paths

pub fn free_path(p: &Path) -> FreeVars {
    match &p.kind {
        PathKind::RootP => empty(),
        PathKind::IdxP(p, x) => add(free_path(p), free_exp(x)),
        PathKind::SliceP(p, a, b) => add(free_path(p), add(free_exp(a), free_exp(b))),
        PathKind::DotP(p, _) => free_path(p),
    }
}

// Arguments

pub fn free_arg(a: &Arg) -> FreeVars {
    match &a.node {
        ArgKind::ExpA(x) => free_exp(x),
        ArgKind::DefA(_) => empty(),
    }
}
pub fn free_args(xs: &[Arg]) -> FreeVars {
    many(xs, free_arg)
}

// Premises

pub fn free_prem(p: &Prem) -> FreeVars {
    match &p.node {
        PremKind::RulePr(_, m, _) | PremKind::IfHoldPr(_, m) | PremKind::IfNotHoldPr(_, m) => m
            .args()
            .into_iter()
            .fold(FreeVars::new(), |a, x| add(a, free_exp(x))),
        PremKind::IfPr(x) | PremKind::DebugPr(x) => free_exp(x),
        PremKind::LetPr(a, b) => add(free_exp(a), free_exp(b)),
        PremKind::IterPr(p, _) => free_prem(p),
    }
}
pub fn free_prems(xs: &[Prem]) -> FreeVars {
    many(xs, free_prem)
}

// Rules

pub fn free_rule(r: &Rule) -> FreeVars {
    add(
        r.node
            .notation
            .args()
            .into_iter()
            .fold(FreeVars::new(), |a, x| add(a, free_exp(x))),
        free_prems(&r.node.premises),
    )
}
pub fn free_rules(xs: &[Rule]) -> FreeVars {
    many(xs, free_rule)
}
pub fn free_rulegroup(g: &RuleGroup) -> FreeVars {
    free_rules(&g.node.1)
}
pub fn free_rulegroups(xs: &[RuleGroup]) -> FreeVars {
    many(xs, free_rulegroup)
}
pub fn free_elsegroup(g: &ElseGroup) -> FreeVars {
    free_rule(&g.node.1)
}
pub fn free_elsegroup_opt(g: &Option<ElseGroup>) -> FreeVars {
    g.as_ref().map_or_else(FreeVars::new, free_elsegroup)
}

// Clauses

pub fn free_clause(c: &Clause) -> FreeVars {
    add(
        free_args(&c.node.args),
        add(free_exp(&c.node.expression), free_prems(&c.node.premises)),
    )
}
pub fn free_clauses(xs: &[Clause]) -> FreeVars {
    many(xs, free_clause)
}
pub fn free_elseclause(c: &ElseClause) -> FreeVars {
    free_clause(c)
}
pub fn free_elseclause_opt(c: &Option<ElseClause>) -> FreeVars {
    c.as_ref().map_or_else(FreeVars::new, free_clause)
}

// Table rows

pub fn free_tablerow(r: &TableRow) -> FreeVars {
    add(free_args(&r.node.0), free_exp(&r.node.1))
}
pub fn free_tablerows(xs: &[TableRow]) -> FreeVars {
    many(xs, free_tablerow)
}

// Definitions

pub fn free_def(d: &Def) -> FreeVars {
    match &d.node {
        DefKind::RelD(_, _, _, gs, e, _) => add(free_rulegroups(gs), free_elsegroup_opt(e)),
        DefKind::TableDecD(_, _, _, rs, _) => free_tablerows(rs),
        DefKind::FuncDecD(_, _, _, _, cs, e, _) => add(free_clauses(cs), free_elseclause_opt(e)),
        _ => empty(),
    }
}
