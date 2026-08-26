//! Free identifiers in intermediate-language data

use crate::domain::sets::{self, IdSet};

use super::ast::*;

// == Free identifiers

fn frees<T>(items: &[T], free: impl Fn(&T) -> IdSet) -> IdSet {
    sets::unions(items.iter().map(free))
}

// - Expressions

/// Collects free identifiers from exp
pub fn free_exp(exp: &Exp) -> IdSet {
    match &exp.node.kind {
        ExpKind::Bool(_) | ExpKind::Num(_) | ExpKind::Text(_) => sets::empty(),
        ExpKind::Var(id) => sets::singleton(id.node.clone()),
        ExpKind::Un(_, _, exp)
        | ExpKind::UpCast(_, exp)
        | ExpKind::DownCast(_, exp)
        | ExpKind::Sub(exp, _, _)
        | ExpKind::Match(exp, _)
        | ExpKind::Len(exp)
        | ExpKind::Dot(exp, _)
        | ExpKind::Iter(exp, _) => free_exp(exp),
        ExpKind::Bin(_, _, exp_l, exp_r)
        | ExpKind::Cmp(_, _, exp_l, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r) => sets::union(free_exp(exp_l), free_exp(exp_r)),
        ExpKind::Tuple(exps) | ExpKind::List(exps) => free_exps(exps),
        ExpKind::Case(not_exp) => not_exp
            .args()
            .into_iter()
            .fold(sets::empty(), |free, exp| sets::union(free, free_exp(exp))),
        ExpKind::Str(fields) => frees(fields, |(_, exp)| free_exp(exp)),
        ExpKind::Opt(Some(exp)) => free_exp(exp),
        ExpKind::Opt(None) => sets::empty(),
        ExpKind::Slice(exp_b, exp_i, exp_n) => sets::union(
            free_exp(exp_b),
            sets::union(free_exp(exp_i), free_exp(exp_n)),
        ),
        ExpKind::Upd(exp_b, path, exp_f) => sets::union(
            free_exp(exp_b),
            sets::union(free_path(path), free_exp(exp_f)),
        ),
        ExpKind::Call(_, _, args) => free_args(args),
    }
}
/// Collects free identifiers from exps
pub fn free_exps(exps: &[Exp]) -> IdSet {
    frees(exps, free_exp)
}

// - Paths

/// Collects free identifiers from path
pub fn free_path(path: &Path) -> IdSet {
    match &path.node.kind {
        PathKind::Root => sets::empty(),
        PathKind::Idx(path, exp_i) => sets::union(free_path(path), free_exp(exp_i)),
        PathKind::Slice(path, exp_i, exp_n) => sets::union(
            free_path(path),
            sets::union(free_exp(exp_i), free_exp(exp_n)),
        ),
        PathKind::Dot(path, _) => free_path(path),
    }
}

// - Arguments

/// Collects free identifiers from arg
pub fn free_arg(arg: &Arg) -> IdSet {
    match &arg.node {
        ArgKind::Exp(exp) => free_exp(exp),
        ArgKind::Def(_) => sets::empty(),
    }
}
/// Collects free identifiers from args
pub fn free_args(args: &[Arg]) -> IdSet {
    frees(args, free_arg)
}

// - Premises

/// Collects free identifiers from prem
pub fn free_prem(prem: &Prem) -> IdSet {
    match &prem.node {
        PremKind::Rule(RulePrem { not_exp, .. })
        | PremKind::IfHold(IfHoldPrem { not_exp, .. })
        | PremKind::IfNotHold(IfNotHoldPrem { not_exp, .. }) => not_exp
            .args()
            .into_iter()
            .fold(sets::empty(), |free, exp| sets::union(free, free_exp(exp))),
        PremKind::If(IfPrem { exp }) | PremKind::Debug(DebugPrem { exp }) => free_exp(exp),
        PremKind::Let(LetPrem { exp_l, exp_r }) => sets::union(free_exp(exp_l), free_exp(exp_r)),
        PremKind::Iter(IteratedPrem { prem, .. }) => free_prem(prem),
    }
}
/// Collects free identifiers from prems
pub fn free_prems(prems: &[Prem]) -> IdSet {
    frees(prems, free_prem)
}

// - Rules

/// Collects free identifiers from rule
pub fn free_rule(rule: &Rule) -> IdSet {
    sets::union(
        rule.node
            .not_exp
            .args()
            .into_iter()
            .fold(sets::empty(), |free, exp| sets::union(free, free_exp(exp))),
        free_prems(&rule.node.prems),
    )
}
/// Collects free identifiers from rules
pub fn free_rules(rules: &[Rule]) -> IdSet {
    frees(rules, free_rule)
}
/// Collects free identifiers from rulegroup
pub fn free_rulegroup(rule_group: &RuleGroup) -> IdSet {
    free_rules(&rule_group.node.1)
}
/// Collects free identifiers from rulegroups
pub fn free_rulegroups(rule_groups: &[RuleGroup]) -> IdSet {
    frees(rule_groups, free_rulegroup)
}
/// Collects free identifiers from elsegroup
pub fn free_elsegroup(else_group: &ElseGroup) -> IdSet {
    free_rule(&else_group.node.1)
}
/// Collects free identifiers from elsegroup opt
pub fn free_elsegroup_opt(else_group: &Option<ElseGroup>) -> IdSet {
    else_group.as_ref().map_or_else(sets::empty, free_elsegroup)
}

// - Clauses

/// Collects free identifiers from clause
pub fn free_clause(clause: &Clause) -> IdSet {
    sets::union(
        free_args(&clause.node.args),
        sets::union(
            free_exp(&clause.node.expression),
            free_prems(&clause.node.premises),
        ),
    )
}
/// Collects free identifiers from clauses
pub fn free_clauses(clauses: &[Clause]) -> IdSet {
    frees(clauses, free_clause)
}
/// Collects free identifiers from elseclause
pub fn free_elseclause(else_clause: &ElseClause) -> IdSet {
    free_clause(else_clause)
}
/// Collects free identifiers from elseclause opt
pub fn free_elseclause_opt(else_clause: &Option<ElseClause>) -> IdSet {
    else_clause.as_ref().map_or_else(sets::empty, free_clause)
}

// - Table rows

/// Collects free identifiers from tablerow
pub fn free_tablerow(table_row: &TableRow) -> IdSet {
    sets::union(free_args(&table_row.node.0), free_exp(&table_row.node.1))
}
/// Collects free identifiers from tablerows
pub fn free_tablerows(table_rows: &[TableRow]) -> IdSet {
    frees(table_rows, free_tablerow)
}

// - Definitions

/// Collects free identifiers from def
pub fn free_def(definition: &Def) -> IdSet {
    match &definition.node {
        DefKind::Rel(Rel {
            rule_groups,
            else_group,
            ..
        }) => sets::union(free_rulegroups(rule_groups), free_elsegroup_opt(else_group)),
        DefKind::TableDec(TableDec {
            rows: table_rows, ..
        }) => free_tablerows(table_rows),
        DefKind::FuncDec(FuncDec {
            clauses,
            else_clause,
            ..
        }) => sets::union(free_clauses(clauses), free_elseclause_opt(else_clause)),
        _ => sets::empty(),
    }
}
