use std::collections::BTreeSet;

use super::ast::*;

pub type IdSet = BTreeSet<IdKind>;
pub type TIdSet = BTreeSet<IdKind>;

fn union<T: Ord>(left: BTreeSet<T>, right: BTreeSet<T>) -> BTreeSet<T> {
    left.into_iter().chain(right).collect()
}

fn ids<T>(items: &[T], free: impl Fn(&T) -> IdSet) -> IdSet {
    items
        .iter()
        .fold(IdSet::new(), |set, item| union(set, free(item)))
}

// Collect free type identifiers

pub fn free_tid_plaintyp(plain_typ: &PlainTyp) -> TIdSet {
    match &plain_typ.node {
        PlainTypKind::BoolT | PlainTypKind::NumT(_) | PlainTypKind::TextT => TIdSet::new(),
        PlainTypKind::VarT(id, targs) => {
            union(TIdSet::from([id.node.clone()]), free_tid_plaintyps(targs))
        }
        PlainTypKind::ParenT(plain_typ) | PlainTypKind::IterT(plain_typ, _) => {
            free_tid_plaintyp(plain_typ)
        }
        PlainTypKind::TupleT(plain_typs) => free_tid_plaintyps(plain_typs),
    }
}

pub fn free_tid_plaintyps(plain_typs: &[PlainTyp]) -> TIdSet {
    ids(plain_typs, free_tid_plaintyp)
}

pub fn free_tid_param(param: &Param) -> TIdSet {
    match &param.node {
        ParamKind::ExpP(plain_typ) => free_tid_plaintyp(plain_typ),
        ParamKind::DefP(_, type_params, params, plain_typ) => union(
            type_params.iter().map(|param| param.node.clone()).collect(),
            union(free_tid_params(params), free_tid_plaintyp(plain_typ)),
        ),
    }
}

pub fn free_tid_params(params: &[Param]) -> TIdSet {
    ids(params, free_tid_param)
}

// Collect free identifiers

// Expressions

pub fn free_id_exp(exp: &Exp) -> IdSet {
    match &exp.node {
        ExpKind::BoolE(_)
        | ExpKind::NumE(_, _)
        | ExpKind::TextE(_)
        | ExpKind::EpsE
        | ExpKind::AtomE(_)
        | ExpKind::HoleE(_)
        | ExpKind::LatexE(_) => IdSet::new(),
        ExpKind::VarE(id) => IdSet::from([id.node.clone()]),
        ExpKind::UnE(_, exp)
        | ExpKind::ArithE(exp)
        | ExpKind::LenE(exp)
        | ExpKind::DotE(exp, _)
        | ExpKind::ParenE(exp)
        | ExpKind::IterE(exp, _)
        | ExpKind::SubE(exp, _)
        | ExpKind::BrackE(_, exp, _)
        | ExpKind::UnparenE(exp) => free_id_exp(exp),
        ExpKind::BinE(left, _, right)
        | ExpKind::CmpE(left, _, right)
        | ExpKind::ConsE(left, right)
        | ExpKind::CatE(left, right)
        | ExpKind::IdxE(left, right)
        | ExpKind::MemE(left, right)
        | ExpKind::InfixE(left, _, right)
        | ExpKind::FuseE(left, right) => union(free_id_exp(left), free_id_exp(right)),
        ExpKind::ListE(exps) | ExpKind::TupleE(exps) | ExpKind::SeqE(exps) => free_id_exps(exps),
        ExpKind::SliceE(base, low, high) => union(
            free_id_exp(base),
            union(free_id_exp(low), free_id_exp(high)),
        ),
        ExpKind::StrE(fields) => ids(fields, |(_, exp)| free_id_exp(exp)),
        ExpKind::UpdE(base, path, field) => union(
            free_id_exp(base),
            union(free_id_path(path), free_id_exp(field)),
        ),
        ExpKind::CallE(_, _, args) => free_id_args(args),
    }
}

pub fn free_id_exps(exps: &[Exp]) -> IdSet {
    ids(exps, free_id_exp)
}

// Paths

pub fn free_id_path(path: &Path) -> IdSet {
    match &path.node {
        PathKind::RootP => IdSet::new(),
        PathKind::IdxP(path, exp) => union(free_id_path(path), free_id_exp(exp)),
        PathKind::SliceP(path, low, high) => union(
            free_id_path(path),
            union(free_id_exp(low), free_id_exp(high)),
        ),
        PathKind::DotP(path, _) => free_id_path(path),
    }
}

// Arguments

pub fn free_id_arg(arg: &Arg) -> IdSet {
    match &arg.node {
        ArgKind::ExpA(exp) => free_id_exp(exp),
        ArgKind::DefA(_) => IdSet::new(),
    }
}

pub fn free_id_args(args: &[Arg]) -> IdSet {
    ids(args, free_id_arg)
}

// Premises

pub fn free_id_prem(prem: &Prem) -> IdSet {
    match &prem.node {
        PremKind::VarPr(id, _) => IdSet::from([id.node.clone()]),
        PremKind::RulePr(_, exp)
        | PremKind::RuleNotPr(_, exp)
        | PremKind::IfPr(exp)
        | PremKind::DebugPr(exp) => free_id_exp(exp),
        PremKind::ElsePr => IdSet::new(),
        PremKind::IterPr(prem, _) => free_id_prem(prem),
    }
}

pub fn free_id_prems(prems: &[Prem]) -> IdSet {
    ids(prems, free_id_prem)
}

// Rules

pub fn free_rule(rule: &Rule) -> IdSet {
    let (_, _, exp, prems) = &rule.node;
    union(free_id_exp(exp), free_id_prems(prems))
}

pub fn free_rules(rules: &[Rule]) -> IdSet {
    ids(rules, free_rule)
}

// Tables

pub fn free_tablerow(table_row: &TableRow) -> IdSet {
    let (pattern, body) = &table_row.node;
    union(free_id_exp(pattern), free_id_exp(body))
}

pub fn free_tablerows(table_rows: &[TableRow]) -> IdSet {
    ids(table_rows, free_tablerow)
}

// Definitions

pub fn free_id_def(definition: &Def) -> IdSet {
    match &definition.node {
        DefKind::RuleGroupD(_, _, rules) => free_rules(rules),
        DefKind::TableDefD(_, table_rows) => free_tablerows(table_rows),
        DefKind::FuncDefD(_, _, args, exp, prems) => union(
            free_id_args(args),
            union(free_id_exp(exp), free_id_prems(prems)),
        ),
        _ => IdSet::new(),
    }
}
