//! Free identifiers in elaboration-language data

use crate::lang::common::ds::set::IdSet;

use super::ast::*;

fn frees<T>(items: &[T], free: impl Fn(&T) -> IdSet) -> IdSet {
    items.iter().map(free).fold(IdSet::new(), IdSet::union)
}

// == Collect free identifiers

// - Expressions

/// Collects free identifiers from id exp
pub fn free_id_exp(exp: &Exp) -> IdSet {
    match &exp.node {
        ExpKind::Bool(_)
        | ExpKind::Num(_, _)
        | ExpKind::Text(_)
        | ExpKind::Eps
        | ExpKind::Atom(_)
        | ExpKind::Hole(_)
        | ExpKind::Latex(_) => IdSet::new(),
        ExpKind::Var(id) => IdSet::from([id.clone()]),
        ExpKind::Un(_, exp)
        | ExpKind::Arith(exp)
        | ExpKind::Len(exp)
        | ExpKind::Dot(exp, _)
        | ExpKind::Paren(exp)
        | ExpKind::Iter(exp, _)
        | ExpKind::Sub(exp, _)
        | ExpKind::Brack(_, exp, _)
        | ExpKind::Unparen(exp) => free_id_exp(exp),
        ExpKind::Bin(exp_l, _, exp_r)
        | ExpKind::Cmp(exp_l, _, exp_r)
        | ExpKind::Cons(exp_l, exp_r)
        | ExpKind::Cat(exp_l, exp_r)
        | ExpKind::Idx(exp_l, exp_r)
        | ExpKind::Mem(exp_l, exp_r)
        | ExpKind::Infix(exp_l, _, exp_r)
        | ExpKind::Fuse(exp_l, exp_r) => free_id_exp(exp_l).union(free_id_exp(exp_r)),
        ExpKind::List(exps) | ExpKind::Tuple(exps) | ExpKind::Seq(exps) => free_id_exps(exps),
        ExpKind::Slice(exp_b, exp_i, exp_n) => free_id_exp(exp_b)
            .union(free_id_exp(exp_i))
            .union(free_id_exp(exp_n)),
        ExpKind::Str(expfields) => frees(expfields, |(_, exp)| free_id_exp(exp)),
        ExpKind::Upd(exp_b, path, exp_f) => free_id_exp(exp_b)
            .union(free_id_path(path))
            .union(free_id_exp(exp_f)),
        ExpKind::Call(_, _, args) => free_id_args(args),
    }
}

/// Collects free identifiers from id exps
pub fn free_id_exps(exps: &[Exp]) -> IdSet {
    frees(exps, free_id_exp)
}

// - Paths

/// Collects free identifiers from id path
pub fn free_id_path(path: &Path) -> IdSet {
    match &path.node {
        PathKind::Root => IdSet::new(),
        PathKind::Idx(path, exp) => free_id_path(path).union(free_id_exp(exp)),
        PathKind::Slice(path, exp_i, exp_n) => free_id_path(path)
            .union(free_id_exp(exp_i))
            .union(free_id_exp(exp_n)),
        PathKind::Dot(path, _) => free_id_path(path),
    }
}

// - Arguments

/// Collects free identifiers from id arg
pub fn free_id_arg(arg: &Arg) -> IdSet {
    match &arg.node {
        ArgKind::Exp(exp) => free_id_exp(exp),
        ArgKind::Def(_) => IdSet::new(),
    }
}

/// Collects free identifiers from id args
pub fn free_id_args(args: &[Arg]) -> IdSet {
    frees(args, free_id_arg)
}

// - Premises

/// Collects free identifiers from id prem
pub fn free_id_prem(prem: &Prem) -> IdSet {
    match &prem.node {
        PremKind::Var(VarPrem { id, .. }) => IdSet::from([id.clone()]),
        PremKind::Rule(RulePrem { exp, .. })
        | PremKind::RuleNot(RuleNotPrem { exp, .. })
        | PremKind::If(IfPrem { exp })
        | PremKind::Debug(DebugPrem { exp }) => free_id_exp(exp),
        PremKind::Else => IdSet::new(),
        PremKind::Iter(IterPrem { prem, .. }) => free_id_prem(prem),
    }
}

/// Collects free identifiers from id prems
pub fn free_id_prems(prems: &[Prem]) -> IdSet {
    frees(prems, free_id_prem)
}

// - Rules

/// Collects free identifiers from id rule
pub fn free_id_rule(rule: &Rule) -> IdSet {
    free_id_exp(&rule.node.2).union(free_id_prems(&rule.node.3))
}

/// Collects free identifiers from id rules
pub fn free_id_rules(rules: &[Rule]) -> IdSet {
    frees(rules, free_id_rule)
}

// - Tables

/// Collects free identifiers from id tablerow
pub fn free_id_tablerow(table_row: &TableRow) -> IdSet {
    let (pattern, exp) = &table_row.node;
    free_id_exp(pattern).union(free_id_exp(exp))
}

/// Collects free identifiers from id tablerows
pub fn free_id_tablerows(table_rows: &[TableRow]) -> IdSet {
    frees(table_rows, free_id_tablerow)
}

// - Definitions

/// Collects free identifiers from id def
pub fn free_id_def(definition: &Def) -> IdSet {
    match &definition.node {
        DefKind::RuleGroup(RuleGroupDef { rules, .. }) => free_id_rules(rules),
        DefKind::TableDef(TableDef { rows, .. }) => free_id_tablerows(rows),
        DefKind::FuncDef(FuncDef {
            args, exp, prems, ..
        }) => free_id_args(args)
            .union(free_id_exp(exp))
            .union(free_id_prems(prems)),
        _ => IdSet::new(),
    }
}
