//! Free identifiers in algorithmic language data

use crate::lang::{common::ds::set::IdSet, il};

use super::ast::*;

// == Free identifiers

fn frees<T>(items: &[T], free: impl Fn(&T) -> IdSet) -> IdSet {
    items.iter().map(free).fold(IdSet::new(), IdSet::union)
}

// - Expressions

/// Collects free identifiers from exp
pub fn free_exp(exp: &Exp) -> IdSet {
    il::free::free_exp(exp)
}

/// Collects free identifiers from exps
pub fn free_exps(exps: &[Exp]) -> IdSet {
    il::free::free_exps(exps)
}

// - Paths

/// Collects free identifiers from path
pub fn free_path(path: &Path) -> IdSet {
    il::free::free_path(path)
}

// - Arguments

/// Collects free identifiers from arg
pub fn free_arg(arg: &Arg) -> IdSet {
    il::free::free_arg(arg)
}

/// Collects free identifiers from args
pub fn free_args(args: &[Arg]) -> IdSet {
    il::free::free_args(args)
}

// - Premises

/// Collects free identifiers from prem
pub fn free_prem(prem: &Prem) -> IdSet {
    il::free::free_prem(prem)
}

/// Collects free identifiers from prems
pub fn free_prems(prems: &[Prem]) -> IdSet {
    il::free::free_prems(prems)
}

// - Rules

/// Collects free identifiers from rulematch
pub fn free_rulematch(rule_match: &RuleMatch) -> IdSet {
    free_exps(&rule_match.exps_signature)
        .union(free_exps(&rule_match.exps_input))
        .union(free_prems(&rule_match.prems))
}

/// Collects free identifiers from rulepath
pub fn free_rulepath(rule_path: &RulePath) -> IdSet {
    free_prems(&rule_path.prems).union(free_exps(&rule_path.exps_output))
}

/// Collects free identifiers from rulepaths
pub fn free_rulepaths(rule_paths: &[RulePath]) -> IdSet {
    frees(rule_paths, free_rulepath)
}

/// Collects free identifiers from rulegroup
pub fn free_rulegroup(rule_group: &RuleGroup) -> IdSet {
    free_rulematch(&rule_group.node.rule_match).union(free_rulepaths(&rule_group.node.rule_paths))
}

/// Collects free identifiers from rulegroups
pub fn free_rulegroups(rule_groups: &[RuleGroup]) -> IdSet {
    frees(rule_groups, free_rulegroup)
}

/// Collects free identifiers from elsegroup
pub fn free_elsegroup(else_group: &ElseGroup) -> IdSet {
    free_rulematch(&else_group.node.rule_match).union(free_rulepath(&else_group.node.rule_path))
}

/// Collects free identifiers from elsegroup opt
pub fn free_elsegroup_opt(else_group: Option<&ElseGroup>) -> IdSet {
    else_group.map_or_else(IdSet::new, free_elsegroup)
}

// - Clauses

/// Collects free identifiers from clause
pub fn free_clause(clause: &Clause) -> IdSet {
    il::free::free_clause(clause)
}

/// Collects free identifiers from clauses
pub fn free_clauses(clauses: &[Clause]) -> IdSet {
    il::free::free_clauses(clauses)
}

/// Collects free identifiers from elseclause
pub fn free_elseclause(else_clause: &ElseClause) -> IdSet {
    il::free::free_elseclause(else_clause)
}

/// Collects free identifiers from elseclause opt
pub fn free_elseclause_opt(else_clause: Option<&ElseClause>) -> IdSet {
    else_clause.map_or_else(IdSet::new, free_elseclause)
}

// - Table rows

/// Collects free identifiers from tablerow
pub fn free_tablerow(table_row: &TableRow) -> IdSet {
    free_args(&table_row.node.args)
        .union(free_exp(&table_row.node.exp))
        .union(free_prems(&table_row.node.prems))
}

/// Collects free identifiers from tablerows
pub fn free_tablerows(table_rows: &[TableRow]) -> IdSet {
    frees(table_rows, free_tablerow)
}

// - Definitions

/// Collects free identifiers from def
pub fn free_def(definition: &Def) -> IdSet {
    match &definition.node {
        DefKind::Rel(RelDef {
            rule_groups,
            else_group,
            ..
        }) => free_rulegroups(rule_groups).union(free_elsegroup_opt(else_group.as_ref())),
        DefKind::TableDec(TableDecDef { table_rows, .. }) => free_tablerows(table_rows),
        DefKind::FuncDec(FuncDecDef {
            clauses,
            else_clause,
            ..
        }) => free_clauses(clauses).union(free_elseclause_opt(else_clause.as_ref())),
        _ => IdSet::new(),
    }
}
