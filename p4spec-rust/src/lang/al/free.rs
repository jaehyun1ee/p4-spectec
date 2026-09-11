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
            .union(self.exp.free())
            .union(self.prems.as_slice().free())
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
    fn free(&self) -> IdSet {
        match self {
            Self::Extern(extern_typ) => extern_typ.free(),
            Self::Defined(defined_typ) => defined_typ.free(),
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
    fn free(&self) -> IdSet {
        self.rule_groups
            .as_slice()
            .free()
            .union(self.else_group.free())
    }
}

impl Free for RelDef {
    fn free(&self) -> IdSet {
        match self {
            Self::Extern(extern_rel) => extern_rel.free(),
            Self::Defined(defined_rel) => defined_rel.free(),
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
    fn free(&self) -> IdSet {
        self.table_rows.as_slice().free()
    }
}

impl Free for DefinedFunc {
    fn free(&self) -> IdSet {
        self.clauses
            .as_slice()
            .free()
            .union(self.else_clause.free())
    }
}

impl Free for MetaFuncDef {
    fn free(&self) -> IdSet {
        match self {
            Self::Extern(extern_func) => extern_func.free(),
            Self::Builtin(builtin_func) => builtin_func.free(),
            Self::Table(table_func) => table_func.free(),
            Self::Defined(defined_func) => defined_func.free(),
        }
    }
}

// - Definitions

impl Free for DefKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Typ(typ_def) => typ_def.free(),
            Self::Var(var_def) => var_def.free(),
            Self::Rel(rel_def) => rel_def.free(),
            Self::MetaFunc(meta_func_def) => meta_func_def.free(),
        }
    }
}

// - Specifications

impl Free for Spec {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}
