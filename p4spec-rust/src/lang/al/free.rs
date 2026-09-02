//! Free identifiers in algorithmic-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Nodes through type arguments alias IL nodes and use their implementations.

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

impl Free for LetPrem {
    fn free_into(&self, free: &mut IdSet) {
        self.exp_l.free_into(free);
        self.exp_r.free_into(free);
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
            Self::Let(prem) => prem.free_into(free),
            Self::Iter(prem) => prem.free_into(free),
            Self::Debug(prem) => prem.free_into(free),
        }
    }
}

// - Rules

impl Free for RuleMatch {
    fn free(&self) -> IdSet {
        self.exps_signature
            .as_slice()
            .free()
            .union(self.exps_input.as_slice().free())
            .union(self.prems.as_slice().free())
    }
}

impl Free for RulePath {
    fn free(&self) -> IdSet {
        self.prems
            .as_slice()
            .free()
            .union(self.exps_output.as_slice().free())
    }
}

impl Free for RuleGroupKind {
    fn free(&self) -> IdSet {
        self.rule_match
            .free()
            .union(self.rule_paths.as_slice().free())
    }
}

impl Free for ElseGroupKind {
    fn free(&self) -> IdSet {
        self.rule_match.free().union(self.rule_path.free())
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

// - Table rows

impl Free for TableRowKind {
    fn free(&self) -> IdSet {
        self.args
            .as_slice()
            .free()
            .union(self.exp.free())
            .union(self.prems.as_slice().free())
    }
}

// Hints alias EL hints and use their implementation.

// - Definitions

impl Free for ExternTypDef {
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

impl Free for ExternRelDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for RelDef {
    fn free(&self) -> IdSet {
        self.rule_groups
            .as_slice()
            .free()
            .union(self.else_group.free())
    }
}

impl Free for ExternDecDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for BuiltinDecDef {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for TableDecDef {
    fn free(&self) -> IdSet {
        self.table_rows.as_slice().free()
    }
}

impl Free for FuncDecDef {
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

// - Specifications

impl Free for Spec {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}
