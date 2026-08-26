//! Free identifiers in elaboration-language data

use crate::domain::sets::{self, IdSet, TIdSet};

use super::ast::*;

fn frees<T>(items: &[T], free: impl Fn(&T) -> IdSet) -> IdSet {
    items
        .iter()
        .fold(IdSet::new(), |set, item| sets::union(set, free(item)))
}

// == Collect free type identifiers

// - Plain types

/// Collects free identifiers from tid plain typ
pub fn free_tid_plain_typ(plain_typ: &PlainTyp) -> TIdSet {
    match &plain_typ.node {
        PlainTypKind::Bool | PlainTypKind::Num(_) | PlainTypKind::Text => TIdSet::new(),
        PlainTypKind::Var(id, targs) => {
            sets::union(TIdSet::from([id.node.clone()]), free_tid_plain_typs(targs))
        }
        PlainTypKind::Paren(plain_typ) | PlainTypKind::Iter(plain_typ, _) => {
            free_tid_plain_typ(plain_typ)
        }
        PlainTypKind::Tuple(plain_typs) => free_tid_plain_typs(plain_typs),
    }
}

/// Collects free identifiers from tid plain typs
pub fn free_tid_plain_typs(plain_typs: &[PlainTyp]) -> TIdSet {
    frees(plain_typs, free_tid_plain_typ)
}

// - Parameters

/// Collects free identifiers from tid param
pub fn free_tid_param(param: &Param) -> TIdSet {
    match &param.node {
        ParamKind::Exp(plain_typ) => free_tid_plain_typ(plain_typ),
        ParamKind::Def(_, tparams, params, plain_typ) => sets::union(
            tparams.iter().map(|tparam| tparam.node.clone()).collect(),
            sets::union(free_tid_params(params), free_tid_plain_typ(plain_typ)),
        ),
    }
}

/// Collects free identifiers from tid params
pub fn free_tid_params(params: &[Param]) -> TIdSet {
    frees(params, free_tid_param)
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
        ExpKind::Var(id) => IdSet::from([id.node.clone()]),
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
        | ExpKind::Fuse(exp_l, exp_r) => sets::union(free_id_exp(exp_l), free_id_exp(exp_r)),
        ExpKind::List(exps) | ExpKind::Tuple(exps) | ExpKind::Seq(exps) => free_id_exps(exps),
        ExpKind::Slice(exp_b, exp_i, exp_n) => sets::union(
            free_id_exp(exp_b),
            sets::union(free_id_exp(exp_i), free_id_exp(exp_n)),
        ),
        ExpKind::Str(expfields) => frees(expfields, |(_, exp)| free_id_exp(exp)),
        ExpKind::Upd(exp_b, path, exp_f) => sets::union(
            free_id_exp(exp_b),
            sets::union(free_id_path(path), free_id_exp(exp_f)),
        ),
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
        PathKind::Idx(path, exp) => sets::union(free_id_path(path), free_id_exp(exp)),
        PathKind::Slice(path, exp_i, exp_n) => sets::union(
            free_id_path(path),
            sets::union(free_id_exp(exp_i), free_id_exp(exp_n)),
        ),
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
        PremKind::Var(VarPrem { id, .. }) => IdSet::from([id.node.clone()]),
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
    sets::union(free_id_exp(&rule.node.2), free_id_prems(&rule.node.3))
}

/// Collects free identifiers from id rules
pub fn free_id_rules(rules: &[Rule]) -> IdSet {
    frees(rules, free_id_rule)
}

// - Tables

/// Collects free identifiers from id tablerow
pub fn free_id_tablerow(table_row: &TableRow) -> IdSet {
    let (pattern, exp) = &table_row.node;
    sets::union(free_id_exp(pattern), free_id_exp(exp))
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
        }) => sets::union(
            free_id_args(args),
            sets::union(free_id_exp(exp), free_id_prems(prems)),
        ),
        _ => IdSet::new(),
    }
}
