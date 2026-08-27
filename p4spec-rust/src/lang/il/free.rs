//! Free identifiers in intermediate-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Numbers, text, identifiers, atoms, and operators alias EL nodes and use their implementations.

// - Mixfix operators

// `Mixop` uses the common implementation.

// - Iterators

impl Free for Iter {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

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
    fn free(&self) -> IdSet {
        match self {
            Self::Struct(fields) => fields.as_slice().free(),
            Self::Case(value_case) => value_case.free(),
            Self::Tuple(values) | Self::List(values) => values.as_slice().free(),
            Self::Opt(value) => value.free(),
            Self::Bool(_) | Self::Num(_) | Self::Text(_) | Self::Func(_) | Self::Extern(_) => {
                IdSet::new()
            }
        }
    }
}

impl Free for ValueField {
    fn free(&self) -> IdSet {
        self.1.free()
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
    fn free(&self) -> IdSet {
        match self {
            Self::Bool(_) | Self::Num(_) | Self::Text(_) => IdSet::new(),
            Self::Var(id) => IdSet::from([id.clone()]),
            Self::Un(_, _, exp)
            | Self::UpCast(_, exp)
            | Self::DownCast(_, exp)
            | Self::Sub(exp, _, _)
            | Self::Match(exp, _)
            | Self::Len(exp)
            | Self::Dot(exp, _)
            | Self::Iter(exp, _) => exp.free(),
            Self::Bin(_, _, exp_l, exp_r)
            | Self::Cmp(_, _, exp_l, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r) => exp_l.free().union(exp_r.free()),
            Self::Tuple(exps) | Self::List(exps) => exps.as_slice().free(),
            Self::Case(not_exp) => not_exp.free(),
            Self::Str(fields) => fields
                .iter()
                .fold(IdSet::new(), |free, (_, exp)| free.union(exp.free())),
            Self::Opt(exp) => exp.free(),
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.free().union(exp_i.free()).union(exp_n.free())
            }
            Self::Upd(exp_b, path, exp_f) => exp_b.free().union(path.free()).union(exp_f.free()),
            Self::Call(_, _, args) => args.as_slice().free(),
        }
    }
}

impl Free for IterExp {
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
    fn free(&self) -> IdSet {
        match self {
            Self::Root => IdSet::new(),
            Self::Idx(path, exp_i) => path.free().union(exp_i.free()),
            Self::Slice(path, exp_i, exp_n) => path.free().union(exp_i.free()).union(exp_n.free()),
            Self::Dot(path, _) => path.free(),
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
    fn free(&self) -> IdSet {
        match self {
            Self::Exp(exp) => exp.free(),
            Self::Def(_) => IdSet::new(),
        }
    }
}

// Type arguments alias types and use the type implementation above.

// - Premises

impl Free for RulePrem {
    fn free(&self) -> IdSet {
        self.not_exp.free()
    }
}

impl Free for IfPrem {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for IfHoldPrem {
    fn free(&self) -> IdSet {
        self.not_exp.free()
    }
}

impl Free for IfNotHoldPrem {
    fn free(&self) -> IdSet {
        self.not_exp.free()
    }
}

impl Free for LetPrem {
    fn free(&self) -> IdSet {
        self.exp_l.free().union(self.exp_r.free())
    }
}

impl Free for IteratedPrem {
    fn free(&self) -> IdSet {
        self.prem.free()
    }
}

impl Free for DebugPrem {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for PremKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Rule(prem) => prem.free(),
            Self::If(prem) => prem.free(),
            Self::IfHold(prem) => prem.free(),
            Self::IfNotHold(prem) => prem.free(),
            Self::Let(prem) => prem.free(),
            Self::Iter(prem) => prem.free(),
            Self::Debug(prem) => prem.free(),
        }
    }
}

impl Free for IterPrem {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Rules

impl Free for RuleKind {
    fn free(&self) -> IdSet {
        self.not_exp.free().union(self.prems.as_slice().free())
    }
}

impl Free for RuleGroupKind {
    fn free(&self) -> IdSet {
        self.1.as_slice().free()
    }
}

impl Free for ElseGroupKind {
    fn free(&self) -> IdSet {
        self.1.free()
    }
}

// - Clauses

impl Free for ClauseKind {
    fn free(&self) -> IdSet {
        self.args
            .as_slice()
            .free()
            .union(self.expression.free())
            .union(self.premises.as_slice().free())
    }
}

// Else clauses alias clauses and use their implementations above.

// - Table rows

impl Free for TableRowKind {
    fn free(&self) -> IdSet {
        self.0.as_slice().free().union(self.1.free())
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
    fn free(&self) -> IdSet {
        self.rule_groups
            .as_slice()
            .free()
            .union(self.else_group.free())
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
    fn free(&self) -> IdSet {
        self.rows.as_slice().free()
    }
}

impl Free for FuncDec {
    fn free(&self) -> IdSet {
        self.clauses
            .as_slice()
            .free()
            .union(self.else_clause.free())
    }
}

impl Free for DefKind {
    fn free(&self) -> IdSet {
        match self {
            Self::ExternTyp(definition) => definition.free(),
            Self::Typ(definition) => definition.free(),
            Self::Var(definition) => definition.free(),
            Self::ExternRel(definition) => definition.free(),
            Self::Rel(definition) => definition.free(),
            Self::ExternDec(definition) => definition.free(),
            Self::BuiltinDec(definition) => definition.free(),
            Self::TableDec(definition) => definition.free(),
            Self::FuncDec(definition) => definition.free(),
        }
    }
}

impl Free for Spec {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}
