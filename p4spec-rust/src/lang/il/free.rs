//! Free identifiers in intermediate-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Numbers, text, identifiers, atoms, and operators alias EL nodes and use their implementations.

// - Mixfix operators

// `Mixop` uses the common implementation.

// - Variables

impl Free for Var {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Types

impl Free for TypKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Subtype checks

impl Free for Subcheck {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Defined types

impl Free for DefTypKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypField {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypOriginKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypCase {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Values

impl Free for ValueKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Struct(fields) => fields.as_slice().free_into(free),
            Self::Case(value_case) => value_case.free_into(free),
            Self::Tuple(values) | Self::List(values) => values.as_slice().free_into(free),
            Self::Opt(value) => value.free_into(free),
            Self::Bool(_) | Self::Num(_) | Self::Text(_) | Self::Func(_) | Self::Extern(_) => {}
        }
    }
}

impl Free for ValueField {
    fn free_into(&self, free: &mut IdSet) {
        self.1.free_into(free);
    }
}

// - Operator types

impl Free for OpTyp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Expressions

impl Free for ExpKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Bool(_) | Self::Num(_) | Self::Text(_) => {}
            Self::Var(id) => {
                free.insert(id.clone());
            }
            Self::Un(_, _, exp)
            | Self::UpCast(_, exp)
            | Self::DownCast(_, exp)
            | Self::Sub(exp, _, _)
            | Self::Match(exp, _)
            | Self::Len(exp)
            | Self::Dot(exp, _)
            | Self::Iter(exp, _) => exp.free_into(free),
            Self::Bin(_, _, exp_l, exp_r)
            | Self::Cmp(_, _, exp_l, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r) => {
                exp_l.free_into(free);
                exp_r.free_into(free);
            }
            Self::Tuple(exps) | Self::List(exps) => exps.as_slice().free_into(free),
            Self::Case(not_exp) => not_exp.free_into(free),
            Self::Str(fields) => {
                for (_, exp) in fields {
                    exp.free_into(free);
                }
            }
            Self::Opt(exp) => exp.free_into(free),
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.free_into(free);
                exp_i.free_into(free);
                exp_n.free_into(free);
            }
            Self::Upd(exp_b, path, exp_f) => {
                exp_b.free_into(free);
                path.free_into(free);
                exp_f.free_into(free);
            }
            Self::Call(_, _, args) => args.as_slice().free_into(free),
        }
    }
}

impl Free for ExpIter {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Patterns

impl Free for Pattern {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for ListPattern {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for OptPattern {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Paths

impl Free for PathKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Root => {}
            Self::Idx(path, exp_i) => {
                path.free_into(free);
                exp_i.free_into(free);
            }
            Self::Slice(path, exp_i, exp_n) => {
                path.free_into(free);
                exp_i.free_into(free);
                exp_n.free_into(free);
            }
            Self::Dot(path, _) => path.free_into(free),
        }
    }
}

// - Parameters

impl Free for ParamKind {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// Type parameters alias identifiers and use the EL identifier implementation.

// - Arguments

impl Free for ArgKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Exp(exp) => exp.free_into(free),
            Self::Def(_) => {}
        }
    }
}

// Type arguments alias types and use the type implementation above.

// - Premises

impl Free for RulePrem {
    fn free_into(&self, free: &mut IdSet) {
        self.not_exp.free_into(free);
    }
}

impl Free for IfPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for IfHoldPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.not_exp.free_into(free);
    }
}

impl Free for IfNotHoldPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.not_exp.free_into(free);
    }
}

impl Free for IterPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.prem.free_into(free);
    }
}

impl Free for DebugPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for PremKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Rule(prem) => prem.free_into(free),
            Self::If(prem) => prem.free_into(free),
            Self::IfHold(prem) => prem.free_into(free),
            Self::IfNotHold(prem) => prem.free_into(free),
            Self::Iter(prem) => prem.free_into(free),
            Self::Debug(prem) => prem.free_into(free),
        }
    }
}

impl Free for PremIter {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Rules

impl Free for RuleKind {
    fn free_into(&self, free: &mut IdSet) {
        self.not_exp.free_into(free);
        self.prems.as_slice().free_into(free);
    }
}

impl Free for RuleGroupKind {
    fn free_into(&self, free: &mut IdSet) {
        self.1.as_slice().free_into(free);
    }
}

impl Free for ElseGroupKind {
    fn free_into(&self, free: &mut IdSet) {
        self.1.free_into(free);
    }
}

// - Clauses

impl Free for ClauseKind {
    fn free_into(&self, free: &mut IdSet) {
        self.args.as_slice().free_into(free);
        self.expression.free_into(free);
        self.premises.as_slice().free_into(free);
    }
}

// Else clauses alias clauses and use their implementations above.

// - Table rows

impl Free for TableRowKind {
    fn free_into(&self, free: &mut IdSet) {
        self.0.as_slice().free_into(free);
        self.1.free_into(free);
    }
}

// Hints alias EL hints and use their implementation.

// - Definitions

impl Free for ExternTyp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for VarDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for ExternRel {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for Rel {
    fn free_into(&self, free: &mut IdSet) {
        self.rule_groups.as_slice().free_into(free);
        self.else_group.free_into(free);
    }
}

impl Free for ExternDec {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for BuiltinDec {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TableDec {
    fn free_into(&self, free: &mut IdSet) {
        self.rows.as_slice().free_into(free);
    }
}

impl Free for FuncDec {
    fn free_into(&self, free: &mut IdSet) {
        self.clauses.as_slice().free_into(free);
        self.else_clause.free_into(free);
    }
}

impl Free for DefKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::ExternTyp(definition) => definition.free_into(free),
            Self::Typ(definition) => definition.free_into(free),
            Self::Var(definition) => definition.free_into(free),
            Self::ExternRel(definition) => definition.free_into(free),
            Self::Rel(definition) => definition.free_into(free),
            Self::ExternDec(definition) => definition.free_into(free),
            Self::BuiltinDec(definition) => definition.free_into(free),
            Self::TableDec(definition) => definition.free_into(free),
            Self::FuncDec(definition) => definition.free_into(free),
        }
    }
}

// - Specifications

impl Free for Spec {
    fn free_into(&self, free: &mut IdSet) {
        self.as_slice().free_into(free);
    }
}
