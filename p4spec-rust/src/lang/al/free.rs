//! Free identifiers in algorithmic language data

use std::collections::BTreeSet;

use super::ast::*;

// Identifier set

pub type FreeVars = BTreeSet<String>;

pub fn empty() -> FreeVars {
    FreeVars::new()
}

pub fn singleton(id: &Id) -> FreeVars {
    [id.node.clone()].into()
}

fn union(mut left: FreeVars, right: FreeVars) -> FreeVars {
    left.extend(right);
    left
}

fn free_notexp(notexp: &NotExp) -> FreeVars {
    notexp.fold(empty(), |free, exp| union(free, free_exp(exp)))
}

// Collect free identifiers

// Expressions

pub fn free_exp(exp: &Exp) -> FreeVars {
    match &exp.kind {
        ExpKind::BoolE(_) | ExpKind::NumE(_) | ExpKind::TextE(_) => empty(),
        ExpKind::VarE(id) => singleton(id),
        ExpKind::UnE(_, _, exp)
        | ExpKind::UpCastE(_, exp)
        | ExpKind::DownCastE(_, exp)
        | ExpKind::LenE(exp)
        | ExpKind::IterE(exp, _) => free_exp(exp),
        ExpKind::BinE(_, _, left, right)
        | ExpKind::CmpE(_, _, left, right)
        | ExpKind::ConsE(left, right)
        | ExpKind::CatE(left, right)
        | ExpKind::MemE(left, right)
        | ExpKind::IdxE(left, right) => union(free_exp(left), free_exp(right)),
        ExpKind::SubE(exp, _, _) | ExpKind::MatchE(exp, _) => free_exp(exp),
        ExpKind::TupleE(exps) | ExpKind::ListE(exps) => free_exps(exps),
        ExpKind::CaseE(notexp) => free_notexp(notexp),
        ExpKind::StrE(fields) => fields
            .iter()
            .fold(empty(), |free, (_, exp)| union(free, free_exp(exp))),
        ExpKind::OptE(Some(exp)) => free_exp(exp),
        ExpKind::OptE(None) => empty(),
        ExpKind::DotE(exp, _) => free_exp(exp),
        ExpKind::SliceE(base, left, right) => {
            union(union(free_exp(base), free_exp(left)), free_exp(right))
        }
        ExpKind::UpdE(base, path, value) => {
            union(union(free_exp(base), free_path(path)), free_exp(value))
        }
        ExpKind::CallE(_, _, args) => free_args(args),
    }
}

pub fn free_exps(exps: &[Exp]) -> FreeVars {
    exps.iter()
        .fold(empty(), |free, exp| union(free, free_exp(exp)))
}

// Paths

pub fn free_path(path: &Path) -> FreeVars {
    match &path.kind {
        PathKind::RootP => empty(),
        PathKind::IdxP(path, exp) => union(free_path(path), free_exp(exp)),
        PathKind::SliceP(path, left, right) => {
            union(union(free_path(path), free_exp(left)), free_exp(right))
        }
        PathKind::DotP(path, _) => free_path(path),
    }
}

// Arguments

pub fn free_arg(arg: &Arg) -> FreeVars {
    match &arg.node {
        ArgKind::ExpA(exp) => free_exp(exp),
        ArgKind::DefA(_) => empty(),
    }
}

pub fn free_args(args: &[Arg]) -> FreeVars {
    args.iter()
        .fold(empty(), |free, arg| union(free, free_arg(arg)))
}

// Premises

pub fn free_prem(prem: &Prem) -> FreeVars {
    match &prem.node {
        PremKind::RulePr(_, notexp, _)
        | PremKind::IfHoldPr(_, notexp)
        | PremKind::IfNotHoldPr(_, notexp) => free_notexp(notexp),
        PremKind::IfPr(exp) | PremKind::DebugPr(exp) => free_exp(exp),
        PremKind::LetPr(left, right) => union(free_exp(left), free_exp(right)),
        PremKind::IterPr(prem, _) => free_prem(prem),
    }
}

pub fn free_prems(prems: &[Prem]) -> FreeVars {
    prems
        .iter()
        .fold(empty(), |free, prem| union(free, free_prem(prem)))
}

// Rules

pub fn free_rulematch(rulematch: &RuleMatch) -> FreeVars {
    union(
        union(
            free_exps(&rulematch.signature),
            free_exps(&rulematch.inputs),
        ),
        free_prems(&rulematch.premises),
    )
}

pub fn free_rulepath(rulepath: &RulePath) -> FreeVars {
    union(free_prems(&rulepath.premises), free_exps(&rulepath.outputs))
}

pub fn free_rulepaths(rulepaths: &[RulePath]) -> FreeVars {
    rulepaths.iter().fold(empty(), |free, rulepath| {
        union(free, free_rulepath(rulepath))
    })
}

pub fn free_rulegroup(rulegroup: &RuleGroup) -> FreeVars {
    union(
        free_rulematch(&rulegroup.node.rule_match),
        free_rulepaths(&rulegroup.node.paths),
    )
}

pub fn free_rulegroups(rulegroups: &[RuleGroup]) -> FreeVars {
    rulegroups
        .iter()
        .fold(empty(), |free, group| union(free, free_rulegroup(group)))
}

pub fn free_elsegroup(elsegroup: &ElseGroup) -> FreeVars {
    union(
        free_rulematch(&elsegroup.node.rule_match),
        free_rulepath(&elsegroup.node.path),
    )
}

pub fn free_elsegroup_opt(elsegroup: Option<&ElseGroup>) -> FreeVars {
    elsegroup.map_or_else(empty, free_elsegroup)
}

// Clauses

pub fn free_clause(clause: &Clause) -> FreeVars {
    union(
        union(
            free_args(&clause.node.args),
            free_exp(&clause.node.expression),
        ),
        free_prems(&clause.node.premises),
    )
}

pub fn free_clauses(clauses: &[Clause]) -> FreeVars {
    clauses
        .iter()
        .fold(empty(), |free, clause| union(free, free_clause(clause)))
}

pub fn free_elseclause(elseclause: &ElseClause) -> FreeVars {
    free_clause(elseclause)
}

pub fn free_elseclause_opt(elseclause: Option<&ElseClause>) -> FreeVars {
    elseclause.map_or_else(empty, free_elseclause)
}

// Table rows

pub fn free_tablerow(tablerow: &TableRow) -> FreeVars {
    union(
        union(
            free_args(&tablerow.node.args),
            free_exp(&tablerow.node.expression),
        ),
        free_prems(&tablerow.node.premises),
    )
}

pub fn free_tablerows(tablerows: &[TableRow]) -> FreeVars {
    tablerows
        .iter()
        .fold(empty(), |free, row| union(free, free_tablerow(row)))
}

// Definitions

pub fn free_def(def: &Def) -> FreeVars {
    match &def.node {
        DefKind::RelD(_, _, _, rulegroups, elsegroup, _) => union(
            free_rulegroups(rulegroups),
            free_elsegroup_opt(elsegroup.as_ref()),
        ),
        DefKind::TableDecD(_, _, _, rows, _) => free_tablerows(rows),
        DefKind::FuncDecD(_, _, _, _, clauses, elseclause, _) => union(
            free_clauses(clauses),
            free_elseclause_opt(elseclause.as_ref()),
        ),
        _ => empty(),
    }
}
