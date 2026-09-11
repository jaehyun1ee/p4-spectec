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

// - Type definitions

impl Free for ExternTyp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for DefinedTyp {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TypDef {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Extern(extern_typ) => extern_typ.free_into(free),
            Self::Defined(defined_typ) => defined_typ.free_into(free),
        }
    }
}

// - Meta-variables

impl Free for VarDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// - Relations

impl Free for ExternRel {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for DefinedRel {
    fn free_into(&self, free: &mut IdSet) {
        self.rule_groups.as_slice().free_into(free);
        self.else_group.free_into(free);
    }
}

impl Free for RelDef {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Extern(extern_rel) => extern_rel.free_into(free),
            Self::Defined(defined_rel) => defined_rel.free_into(free),
        }
    }
}

// - Meta-functions

impl Free for ExternFunc {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for BuiltinFunc {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TableFunc {
    fn free_into(&self, free: &mut IdSet) {
        self.rows.as_slice().free_into(free);
    }
}

impl Free for DefinedFunc {
    fn free_into(&self, free: &mut IdSet) {
        self.clauses.as_slice().free_into(free);
        self.else_clause.free_into(free);
    }
}

impl Free for MetaFuncDef {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Extern(extern_func) => extern_func.free_into(free),
            Self::Builtin(builtin_func) => builtin_func.free_into(free),
            Self::Table(table_func) => table_func.free_into(free),
            Self::Defined(defined_func) => defined_func.free_into(free),
        }
    }
}

// - Definitions

impl Free for DefKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::Typ(typ_def) => typ_def.free_into(free),
            Self::Var(var_def) => var_def.free_into(free),
            Self::Rel(rel_def) => rel_def.free_into(free),
            Self::MetaFunc(meta_func_def) => meta_func_def.free_into(free),
        }
    }
}

// - Specifications

impl Free for Spec {
    fn free_into(&self, free: &mut IdSet) {
        self.as_slice().free_into(free);
    }
}
