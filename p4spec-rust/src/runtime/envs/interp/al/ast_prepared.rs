//! Slot instantiation of shared AL syntax

use std::rc::Rc;

pub use crate::interp::shared::prepare::expr::*;
use crate::interp::shared::prepare::{Prepare, restore_phrase};
use crate::lang::al::ast as source;
pub use crate::lang::al::ast::{
    BuiltinFunc, DefinedTyp, ExternFunc, ExternRel, ExternTyp, TypDef, VarDef,
};
use crate::lang::data::var::{IdSlot, VarSlot};
use crate::runtime::envs::interp::shared::frame::{Callable, FrameLayout};

// == Prepared syntax

// - Premises

pub type Prem = source::Prem<IdSlot, VarSlot>;
pub type PremKind = source::PremKind<IdSlot, VarSlot>;
pub type RulePrem = source::RulePrem<IdSlot, VarSlot>;
pub type IfPrem = source::IfPrem<IdSlot, VarSlot>;
pub type IfHoldPrem = source::IfHoldPrem<IdSlot, VarSlot>;
pub type IfNotHoldPrem = source::IfNotHoldPrem<IdSlot, VarSlot>;
pub type LetPrem = source::LetPrem<IdSlot, VarSlot>;
pub type IterPrem = source::IterPrem<IdSlot, VarSlot>;
pub type DebugPrem = source::DebugPrem<IdSlot, VarSlot>;

// - Rules

pub type RuleMatch = source::RuleMatch<IdSlot, VarSlot>;
pub type RulePath = source::RulePath<IdSlot, VarSlot>;
pub type RuleGroup = source::RuleGroup<IdSlot, VarSlot>;
pub type RuleGroupKind = source::RuleGroupKind<IdSlot, VarSlot>;
pub type ElseGroup = source::ElseGroup<IdSlot, VarSlot>;
pub type ElseGroupKind = source::ElseGroupKind<IdSlot, VarSlot>;

// - Clauses

pub type Clause = source::Clause<IdSlot, VarSlot>;
pub type ClauseKind = source::ClauseKind<IdSlot, VarSlot>;
pub type ElseClause = source::ElseClause<IdSlot, VarSlot>;
pub type ElseClauseKind = source::ElseClauseKind<IdSlot, VarSlot>;

// - Table rows

pub type TableRow = source::TableRow<IdSlot, VarSlot>;
pub type TableRowKind = source::TableRowKind<IdSlot, VarSlot>;

// - Relation definitions

pub type RelDef = source::RelDef<IdSlot, VarSlot>;
pub type DefinedRel = source::DefinedRel<IdSlot, VarSlot>;

// - Meta-function definitions

pub type MetaFuncDef = source::MetaFuncDef<IdSlot, VarSlot>;
pub type TableFunc = source::TableFunc<IdSlot, VarSlot>;
pub type DefinedFunc = source::DefinedFunc<IdSlot, VarSlot>;

// - Definitions

pub type Def = source::Def<IdSlot, VarSlot>;
pub type DefKind = source::DefKind<IdSlot, VarSlot>;
pub type Spec = Vec<Def>;

// == Preparation

// - Relation definitions

pub fn prepare_rel_def(rel_source: source::RelDef) -> Callable<RelDef> {
    let mut layout = FrameLayout::default();
    let rel = rel_source.prepare(&mut layout);
    Callable { def: rel, layout: Rc::new(layout) }
}

// - Meta-function definitions

pub fn prepare_func_def(func_source: source::MetaFuncDef) -> Callable<MetaFuncDef> {
    let mut layout = FrameLayout::default();
    let func = func_source.prepare(&mut layout);
    Callable { def: func, layout: Rc::new(layout) }
}

// == Preparation traversal

// == Premises

impl Prepare for source::PremKind {
    type Output = PremKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::PremKind::Rule(prem_inner) => PremKind::Rule(prem_inner.prepare(layout)),
            source::PremKind::If(prem_inner) => PremKind::If(prem_inner.prepare(layout)),
            source::PremKind::IfHold(prem_inner) => PremKind::IfHold(prem_inner.prepare(layout)),
            source::PremKind::IfNotHold(prem_inner) => {
                PremKind::IfNotHold(prem_inner.prepare(layout))
            }
            source::PremKind::Let(prem_inner) => PremKind::Let(prem_inner.prepare(layout)),
            source::PremKind::Iter(prem_inner) => PremKind::Iter(prem_inner.prepare(layout)),
            source::PremKind::Debug(prem_inner) => PremKind::Debug(prem_inner.prepare(layout)),
        }
    }
}

// - Rule premise

impl Prepare for source::RulePrem {
    type Output = RulePrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        RulePrem { id: self.id, not_exp: self.not_exp.prepare(layout), input_hint: self.input_hint }
    }
}

// - If premise

impl Prepare for source::IfPrem {
    type Output = IfPrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        IfPrem { exp: self.exp.prepare(layout) }
    }
}

// - If-hold premise

impl Prepare for source::IfHoldPrem {
    type Output = IfHoldPrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        IfHoldPrem { id: self.id, not_exp: self.not_exp.prepare(layout) }
    }
}

// - If-not-hold premise

impl Prepare for source::IfNotHoldPrem {
    type Output = IfNotHoldPrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        IfNotHoldPrem { id: self.id, not_exp: self.not_exp.prepare(layout) }
    }
}

// - Let premise

impl Prepare for source::LetPrem {
    type Output = LetPrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        LetPrem { exp_l: self.exp_l.prepare(layout), exp_r: self.exp_r.prepare(layout) }
    }
}

// - Iterated premise

impl Prepare for source::IterPrem {
    type Output = IterPrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        IterPrem { prem: self.prem.prepare(layout), prem_iter: self.prem_iter.prepare(layout) }
    }
}

// - Debug premise

impl Prepare for source::DebugPrem {
    type Output = DebugPrem;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        DebugPrem { exp: self.exp.prepare(layout) }
    }
}

// == Rules

// - Rule match

impl Prepare for source::RuleMatch {
    type Output = RuleMatch;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        RuleMatch {
            exps_signature: self.exps_signature.prepare(layout),
            exps_input: self.exps_input.prepare(layout),
            prems: self.prems.prepare(layout),
        }
    }
}

// - Rule path

impl Prepare for source::RulePath {
    type Output = RulePath;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        RulePath {
            id: self.id,
            prems: self.prems.prepare(layout),
            exps_output: self.exps_output.prepare(layout),
        }
    }
}

// - Rule group

impl Prepare for source::RuleGroupKind {
    type Output = RuleGroupKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        RuleGroupKind {
            id: self.id,
            rule_match: self.rule_match.prepare(layout),
            rule_paths: self.rule_paths.prepare(layout),
        }
    }
}

// - Else group

impl Prepare for source::ElseGroupKind {
    type Output = ElseGroupKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        ElseGroupKind {
            id: self.id,
            rule_match: self.rule_match.prepare(layout),
            rule_path: self.rule_path.prepare(layout),
        }
    }
}

// == Clauses

impl Prepare for source::ClauseKind {
    type Output = ClauseKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        ClauseKind {
            args: self.args.prepare(layout),
            exp: self.exp.prepare(layout),
            prems: self.prems.prepare(layout),
        }
    }
}

// == Table rows

impl Prepare for source::TableRowKind {
    type Output = TableRowKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        TableRowKind {
            exps_signature: self.exps_signature.prepare(layout),
            args: self.args.prepare(layout),
            exp: self.exp.prepare(layout),
            prems: self.prems.prepare(layout),
        }
    }
}

// == Relation definitions

// - Relation definition

impl Prepare for source::RelDef {
    type Output = RelDef;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::RelDef::Extern(rel_inner) => RelDef::Extern(rel_inner),
            source::RelDef::Defined(rel_inner) => RelDef::Defined(rel_inner.prepare(layout)),
        }
    }
}

// - Defined relation definition

impl Prepare for source::DefinedRel {
    type Output = DefinedRel;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        DefinedRel {
            id: self.id,
            not_typ: self.not_typ,
            input_hint: self.input_hint,
            rule_groups: self.rule_groups.prepare(layout),
            else_group: self.else_group.prepare(layout),
            hints: self.hints,
        }
    }
}

// == Meta-function definitions

// - Meta-function definition

impl Prepare for source::MetaFuncDef {
    type Output = MetaFuncDef;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::MetaFuncDef::Extern(func_inner) => MetaFuncDef::Extern(func_inner),
            source::MetaFuncDef::Builtin(func_inner) => MetaFuncDef::Builtin(func_inner),
            source::MetaFuncDef::Table(func_inner) => {
                MetaFuncDef::Table(func_inner.prepare(layout))
            }
            source::MetaFuncDef::Defined(func_inner) => {
                MetaFuncDef::Defined(func_inner.prepare(layout))
            }
        }
    }
}

// - Table function definition

impl Prepare for source::TableFunc {
    type Output = TableFunc;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        TableFunc {
            id: self.id,
            params: self.params,
            typ: self.typ,
            table_rows: self.table_rows.prepare(layout),
            hints: self.hints,
        }
    }
}

// - Defined function definition

impl Prepare for source::DefinedFunc {
    type Output = DefinedFunc;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        DefinedFunc {
            id: self.id,
            tparams: self.tparams,
            params: self.params,
            typ: self.typ,
            clauses: self.clauses.prepare(layout),
            else_clause: self.else_clause.prepare(layout),
            hints: self.hints,
        }
    }
}

// == Definitions

impl Prepare for source::DefKind {
    type Output = DefKind;

    fn prepare(self, layout: &mut FrameLayout) -> Self::Output {
        match self {
            source::DefKind::Typ(typdef) => DefKind::Typ(typdef),
            source::DefKind::Var(def_var) => DefKind::Var(def_var),
            source::DefKind::Rel(rel_inner) => DefKind::Rel(rel_inner.prepare(layout)),
            source::DefKind::MetaFunc(func_inner) => DefKind::MetaFunc(func_inner.prepare(layout)),
        }
    }
}

// == Source reconstruction

pub fn restore_prem(prem: Prem) -> source::Prem {
    restore_phrase(prem, restore_prem_kind)
}

pub fn restore_rule_group(rule_group: RuleGroup) -> source::RuleGroup {
    restore_phrase(rule_group, restore_rule_group_kind)
}

pub fn restore_else_group(else_group: ElseGroup) -> source::ElseGroup {
    restore_phrase(else_group, restore_else_group_kind)
}

pub fn restore_clause(clause: Clause) -> source::Clause {
    restore_phrase(clause, restore_clause_kind)
}

pub fn restore_else_clause(else_clause: ElseClause) -> source::ElseClause {
    restore_clause(else_clause)
}

pub fn restore_else_clause_kind(else_clause_kind: ElseClauseKind) -> source::ElseClauseKind {
    restore_clause_kind(else_clause_kind)
}

pub fn restore_table_row(table_row: TableRow) -> source::TableRow {
    restore_phrase(table_row, restore_table_row_kind)
}

pub fn restore_def(def: Def) -> source::Def {
    restore_phrase(def, restore_def_kind)
}

pub fn restore_prem_kind(prem_kind: PremKind) -> source::PremKind {
    match prem_kind {
        PremKind::Rule(prem_inner) => source::PremKind::Rule(restore_rule_prem(prem_inner)),
        PremKind::If(prem_inner) => source::PremKind::If(restore_if_prem(prem_inner)),
        PremKind::IfHold(prem_inner) => source::PremKind::IfHold(restore_if_hold_prem(prem_inner)),
        PremKind::IfNotHold(prem_inner) => {
            source::PremKind::IfNotHold(restore_if_not_hold_prem(prem_inner))
        }
        PremKind::Let(prem_inner) => source::PremKind::Let(restore_let_prem(prem_inner)),
        PremKind::Iter(prem_inner) => source::PremKind::Iter(restore_iter_prem(prem_inner)),
        PremKind::Debug(prem_inner) => source::PremKind::Debug(restore_debug_prem(prem_inner)),
    }
}

pub fn restore_rule_prem(rule_prem: RulePrem) -> source::RulePrem {
    source::RulePrem {
        id: rule_prem.id,
        not_exp: restore_not_exp(rule_prem.not_exp),
        input_hint: rule_prem.input_hint,
    }
}

pub fn restore_if_prem(if_prem: IfPrem) -> source::IfPrem {
    source::IfPrem { exp: restore_exp(if_prem.exp) }
}

pub fn restore_if_hold_prem(if_hold_prem: IfHoldPrem) -> source::IfHoldPrem {
    source::IfHoldPrem { id: if_hold_prem.id, not_exp: restore_not_exp(if_hold_prem.not_exp) }
}

pub fn restore_if_not_hold_prem(if_not_hold_prem: IfNotHoldPrem) -> source::IfNotHoldPrem {
    source::IfNotHoldPrem {
        id: if_not_hold_prem.id,
        not_exp: restore_not_exp(if_not_hold_prem.not_exp),
    }
}

pub fn restore_let_prem(let_prem: LetPrem) -> source::LetPrem {
    source::LetPrem { exp_l: restore_exp(let_prem.exp_l), exp_r: restore_exp(let_prem.exp_r) }
}

pub fn restore_iter_prem(iter_prem: IterPrem) -> source::IterPrem {
    source::IterPrem {
        prem: Box::new(restore_prem(*iter_prem.prem)),
        prem_iter: restore_prem_iter(iter_prem.prem_iter),
    }
}

pub fn restore_debug_prem(debug_prem: DebugPrem) -> source::DebugPrem {
    source::DebugPrem { exp: restore_exp(debug_prem.exp) }
}

pub fn restore_rule_match(rule_match: RuleMatch) -> source::RuleMatch {
    source::RuleMatch {
        exps_signature: rule_match
            .exps_signature
            .into_iter()
            .map(restore_exp)
            .collect(),
        exps_input: rule_match.exps_input.into_iter().map(restore_exp).collect(),
        prems: rule_match.prems.into_iter().map(restore_prem).collect(),
    }
}

pub fn restore_rule_path(rule_path: RulePath) -> source::RulePath {
    source::RulePath {
        id: rule_path.id,
        prems: rule_path.prems.into_iter().map(restore_prem).collect(),
        exps_output: rule_path.exps_output.into_iter().map(restore_exp).collect(),
    }
}

pub fn restore_rule_group_kind(rule_group_kind: RuleGroupKind) -> source::RuleGroupKind {
    source::RuleGroupKind {
        id: rule_group_kind.id,
        rule_match: restore_rule_match(rule_group_kind.rule_match),
        rule_paths: rule_group_kind
            .rule_paths
            .into_iter()
            .map(restore_rule_path)
            .collect(),
    }
}

pub fn restore_else_group_kind(else_group_kind: ElseGroupKind) -> source::ElseGroupKind {
    source::ElseGroupKind {
        id: else_group_kind.id,
        rule_match: restore_rule_match(else_group_kind.rule_match),
        rule_path: restore_rule_path(else_group_kind.rule_path),
    }
}

pub fn restore_clause_kind(clause_kind: ClauseKind) -> source::ClauseKind {
    source::ClauseKind {
        args: clause_kind.args.into_iter().map(restore_arg).collect(),
        exp: restore_exp(clause_kind.exp),
        prems: clause_kind.prems.into_iter().map(restore_prem).collect(),
    }
}

pub fn restore_table_row_kind(table_row_kind: TableRowKind) -> source::TableRowKind {
    source::TableRowKind {
        exps_signature: table_row_kind
            .exps_signature
            .into_iter()
            .map(restore_exp)
            .collect(),
        args: table_row_kind.args.into_iter().map(restore_arg).collect(),
        exp: restore_exp(table_row_kind.exp),
        prems: table_row_kind.prems.into_iter().map(restore_prem).collect(),
    }
}

pub fn restore_rel(rel_def: RelDef) -> source::RelDef {
    match rel_def {
        RelDef::Extern(rel_inner) => source::RelDef::Extern(rel_inner),
        RelDef::Defined(rel_inner) => {
            source::RelDef::Defined(Box::new(restore_defined_rel(*rel_inner)))
        }
    }
}

pub fn restore_defined_rel(defined_rel: DefinedRel) -> source::DefinedRel {
    source::DefinedRel {
        id: defined_rel.id,
        not_typ: defined_rel.not_typ,
        input_hint: defined_rel.input_hint,
        rule_groups: defined_rel
            .rule_groups
            .into_iter()
            .map(restore_rule_group)
            .collect(),
        else_group: defined_rel.else_group.map(restore_else_group),
        hints: defined_rel.hints,
    }
}

pub fn restore_func(meta_func_def: MetaFuncDef) -> source::MetaFuncDef {
    match meta_func_def {
        MetaFuncDef::Extern(func_inner) => source::MetaFuncDef::Extern(func_inner),
        MetaFuncDef::Builtin(func_inner) => source::MetaFuncDef::Builtin(func_inner),
        MetaFuncDef::Table(func_inner) => {
            source::MetaFuncDef::Table(restore_table_func(func_inner))
        }
        MetaFuncDef::Defined(func_inner) => {
            source::MetaFuncDef::Defined(Box::new(restore_defined_func(*func_inner)))
        }
    }
}

pub fn restore_table_func(table_func: TableFunc) -> source::TableFunc {
    source::TableFunc {
        id: table_func.id,
        params: table_func.params,
        typ: table_func.typ,
        table_rows: table_func
            .table_rows
            .into_iter()
            .map(restore_table_row)
            .collect(),
        hints: table_func.hints,
    }
}

pub fn restore_defined_func(defined_func: DefinedFunc) -> source::DefinedFunc {
    source::DefinedFunc {
        id: defined_func.id,
        tparams: defined_func.tparams,
        params: defined_func.params,
        typ: defined_func.typ,
        clauses: defined_func
            .clauses
            .into_iter()
            .map(restore_clause)
            .collect(),
        else_clause: defined_func.else_clause.map(restore_else_clause),
        hints: defined_func.hints,
    }
}

pub fn restore_def_kind(def_kind: DefKind) -> source::DefKind {
    match def_kind {
        DefKind::Typ(typdef) => source::DefKind::Typ(typdef),
        DefKind::Var(def_var) => source::DefKind::Var(def_var),
        DefKind::Rel(rel_inner) => source::DefKind::Rel(restore_rel(rel_inner)),
        DefKind::MetaFunc(func_inner) => source::DefKind::MetaFunc(restore_func(func_inner)),
    }
}

pub fn restore_rel_def(rel: Callable<RelDef>) -> source::RelDef {
    restore_rel(rel.def)
}

pub fn restore_func_def(func: Callable<MetaFuncDef>) -> source::MetaFuncDef {
    restore_func(func.def)
}
