//! Free identifiers in internal language data

use std::collections::BTreeSet;

use super::ast::*;

// Identifier set

pub type T = BTreeSet<IdKind>;
pub fn empty() -> T {
    T::new()
}
pub fn singleton(id: &Id) -> T {
    T::from([id.node.clone()])
}
fn add(a: T, b: T) -> T {
    a.into_iter().chain(b).collect()
}
fn many<TItem>(xs: &[TItem], f: impl Fn(&TItem) -> T) -> T {
    xs.iter().fold(T::new(), |a, x| add(a, f(x)))
}

// Collect free identifiers

// Expressions

pub fn free_exp(exp: &Exp) -> T {
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
            .fold(T::new(), |a, x| add(a, free_exp(x))),
        ExpKind::StrE(xs) => many(xs, |(_, x)| free_exp(x)),
        ExpKind::OptE(Some(x)) => free_exp(x),
        ExpKind::OptE(None) => empty(),
        ExpKind::SliceE(a, b, c) => add(free_exp(a), add(free_exp(b), free_exp(c))),
        ExpKind::UpdE(a, p, b) => add(free_exp(a), add(free_path(p), free_exp(b))),
        ExpKind::CallE(_, _, args) => free_args(args),
    }
}
pub fn free_exps(xs: &[Exp]) -> T {
    many(xs, free_exp)
}

// Paths

pub fn free_path(p: &Path) -> T {
    match &p.kind {
        PathKind::RootP => empty(),
        PathKind::IdxP(p, x) => add(free_path(p), free_exp(x)),
        PathKind::SliceP(p, a, b) => add(free_path(p), add(free_exp(a), free_exp(b))),
        PathKind::DotP(p, _) => free_path(p),
    }
}

// Arguments

pub fn free_arg(a: &Arg) -> T {
    match &a.node {
        ArgKind::ExpA(x) => free_exp(x),
        ArgKind::DefA(_) => empty(),
    }
}
pub fn free_args(xs: &[Arg]) -> T {
    many(xs, free_arg)
}

// Premises

pub fn free_prem(p: &Prem) -> T {
    match &p.node {
        PremKind::RulePr(_, m, _) | PremKind::IfHoldPr(_, m) | PremKind::IfNotHoldPr(_, m) => m
            .args()
            .into_iter()
            .fold(T::new(), |a, x| add(a, free_exp(x))),
        PremKind::IfPr(x) | PremKind::DebugPr(x) => free_exp(x),
        PremKind::LetPr(a, b) => add(free_exp(a), free_exp(b)),
        PremKind::IterPr(p, _) => free_prem(p),
    }
}
pub fn free_prems(xs: &[Prem]) -> T {
    many(xs, free_prem)
}

// Rules

pub fn free_rule(r: &Rule) -> T {
    let (_, m, p) = &r.node;
    add(
        m.args()
            .into_iter()
            .fold(T::new(), |a, x| add(a, free_exp(x))),
        free_prems(p),
    )
}
pub fn free_rules(xs: &[Rule]) -> T {
    many(xs, free_rule)
}
pub fn free_rulegroup(g: &RuleGroup) -> T {
    free_rules(&g.node.1)
}
pub fn free_rulegroups(xs: &[RuleGroup]) -> T {
    many(xs, free_rulegroup)
}
pub fn free_elsegroup(g: &ElseGroup) -> T {
    free_rule(&g.node.1)
}
pub fn free_elsegroup_opt(g: &Option<ElseGroup>) -> T {
    g.as_ref().map_or_else(T::new, free_elsegroup)
}

// Clauses

pub fn free_clause(c: &Clause) -> T {
    let (a, x, p) = &c.node;
    add(free_args(a), add(free_exp(x), free_prems(p)))
}
pub fn free_clauses(xs: &[Clause]) -> T {
    many(xs, free_clause)
}
pub fn free_elseclause(c: &ElseClause) -> T {
    free_clause(c)
}
pub fn free_elseclause_opt(c: &Option<ElseClause>) -> T {
    c.as_ref().map_or_else(T::new, free_clause)
}

// Table rows

pub fn free_tablerow(r: &TableRow) -> T {
    add(free_args(&r.node.0), free_exp(&r.node.1))
}
pub fn free_tablerows(xs: &[TableRow]) -> T {
    many(xs, free_tablerow)
}

// Definitions

pub fn free_def(d: &Def) -> T {
    match &d.node {
        DefKind::RelD(_, _, _, gs, e, _) => add(free_rulegroups(gs), free_elsegroup_opt(e)),
        DefKind::TableDecD(_, _, _, rs, _) => free_tablerows(rs),
        DefKind::FuncDecD(_, _, _, _, cs, e, _) => add(free_clauses(cs), free_elseclause_opt(e)),
        _ => empty(),
    }
}
