//! Free identifiers in intermediate-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Numbers, text, identifiers, atoms, and operators alias EL nodes and use their implementations.

// - Mixfix operators

// `Mixop` uses the common implementation.

// - Iterators

impl Free for Iter {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Variables

impl Free for Var {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Types

impl Free for TypKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Subtype checks

impl Free for Subcheck {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Defined types

impl Free for DefTypKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypField {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypOriginKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypCase {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Values

impl Free for ValueKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Struct(fields) => fields.as_slice().collect_free(free),
            Self::Case(value_case) => value_case.collect_free(free),
            Self::Tuple(values) | Self::List(values) => values.as_slice().collect_free(free),
            Self::Opt(value) => value.collect_free(free),
            Self::Bool(_) | Self::Num(_) | Self::Text(_) | Self::Func(_) | Self::Extern(_) => {}
        }
    }
}

impl Free for ValueField {
    fn collect_free(&self, free: &mut IdSet) {
        self.1.collect_free(free);
    }
}

// - Operator types

impl Free for OpTyp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Expressions

impl Free for ExpKind {
    fn collect_free(&self, free: &mut IdSet) {
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
            | Self::Iter(exp, _) => exp.collect_free(free),
            Self::Bin(_, _, exp_l, exp_r)
            | Self::Cmp(_, _, exp_l, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r) => {
                exp_l.collect_free(free);
                exp_r.collect_free(free);
            }
            Self::Tuple(exps) | Self::List(exps) => exps.as_slice().collect_free(free),
            Self::Case(not_exp) => not_exp.collect_free(free),
            Self::Str(fields) => {
                for (_, exp) in fields {
                    exp.collect_free(free);
                }
            }
            Self::Opt(exp) => exp.collect_free(free),
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.collect_free(free);
                exp_i.collect_free(free);
                exp_n.collect_free(free);
            }
            Self::Upd(exp_b, path, exp_f) => {
                exp_b.collect_free(free);
                path.collect_free(free);
                exp_f.collect_free(free);
            }
            Self::Call(_, _, args) => args.as_slice().collect_free(free),
        }
    }
}

impl Free for IterExp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Patterns

impl Free for Pattern {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for ListPattern {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for OptPattern {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Paths

impl Free for PathKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Root => {}
            Self::Idx(path, exp_i) => {
                path.collect_free(free);
                exp_i.collect_free(free);
            }
            Self::Slice(path, exp_i, exp_n) => {
                path.collect_free(free);
                exp_i.collect_free(free);
                exp_n.collect_free(free);
            }
            Self::Dot(path, _) => path.collect_free(free),
        }
    }
}

// - Parameters

impl Free for ParamKind {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// Type parameters alias identifiers and use the EL identifier implementation.

// - Arguments

impl Free for ArgKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Exp(exp) => exp.collect_free(free),
            Self::Def(_) => {}
        }
    }
}

// Type arguments alias types and use the type implementation above.

// - Premises

impl Free for RulePrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
    }
}

impl Free for IfPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for IfHoldPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
    }
}

impl Free for IfNotHoldPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
    }
}

impl Free for LetPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp_l.collect_free(free);
        self.exp_r.collect_free(free);
    }
}

impl Free for IteratedPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.prem.collect_free(free);
    }
}

impl Free for DebugPrem {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for PremKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Rule(prem) => prem.collect_free(free),
            Self::If(prem) => prem.collect_free(free),
            Self::IfHold(prem) => prem.collect_free(free),
            Self::IfNotHold(prem) => prem.collect_free(free),
            Self::Let(prem) => prem.collect_free(free),
            Self::Iter(prem) => prem.collect_free(free),
            Self::Debug(prem) => prem.collect_free(free),
        }
    }
}

impl Free for IterPrem {
    fn collect_free(&self, _free: &mut IdSet) {}
}

// - Rules

impl Free for RuleKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
        self.prems.as_slice().collect_free(free);
    }
}

impl Free for RuleGroupKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.1.as_slice().collect_free(free);
    }
}

impl Free for ElseGroupKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.1.collect_free(free);
    }
}

// - Clauses

impl Free for ClauseKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.args.as_slice().collect_free(free);
        self.expression.collect_free(free);
        self.premises.as_slice().collect_free(free);
    }
}

// Else clauses alias clauses and use their implementations above.

// - Table rows

impl Free for TableRowKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.0.as_slice().collect_free(free);
        self.1.collect_free(free);
    }
}

// Hints alias EL hints and use their implementation.

// - Definitions

impl Free for ExternTyp {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for VarDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for ExternRel {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for Rel {
    fn collect_free(&self, free: &mut IdSet) {
        self.rule_groups.as_slice().collect_free(free);
        self.else_group.collect_free(free);
    }
}

impl Free for ExternDec {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for BuiltinDec {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TableDec {
    fn collect_free(&self, free: &mut IdSet) {
        self.rows.as_slice().collect_free(free);
    }
}

impl Free for FuncDec {
    fn collect_free(&self, free: &mut IdSet) {
        self.clauses.as_slice().collect_free(free);
        self.else_clause.collect_free(free);
    }
}

impl Free for DefKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::ExternTyp(definition) => definition.collect_free(free),
            Self::Typ(definition) => definition.collect_free(free),
            Self::Var(definition) => definition.collect_free(free),
            Self::ExternRel(definition) => definition.collect_free(free),
            Self::Rel(definition) => definition.collect_free(free),
            Self::ExternDec(definition) => definition.collect_free(free),
            Self::BuiltinDec(definition) => definition.collect_free(free),
            Self::TableDec(definition) => definition.collect_free(free),
            Self::FuncDec(definition) => definition.collect_free(free),
        }
    }
}

// - Specifications

impl Free for Spec {
    fn collect_free(&self, free: &mut IdSet) {
        self.as_slice().collect_free(free);
    }
}
