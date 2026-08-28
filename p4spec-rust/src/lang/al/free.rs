//! Free identifiers in algorithmic-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Nodes through premises alias IL nodes and use their implementations.

// - Rules

impl Free for RuleMatch {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_signature.as_slice().collect_free(free);
        self.exps_input.as_slice().collect_free(free);
        self.prems.as_slice().collect_free(free);
    }
}

impl Free for RulePath {
    fn collect_free(&self, free: &mut IdSet) {
        self.prems.as_slice().collect_free(free);
        self.exps_output.as_slice().collect_free(free);
    }
}

impl Free for RuleGroupKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.rule_match.collect_free(free);
        self.rule_paths.as_slice().collect_free(free);
    }
}

impl Free for ElseGroupKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.rule_match.collect_free(free);
        self.rule_path.collect_free(free);
    }
}

// Clauses alias IL clauses and use their implementations.

// - Table rows

impl Free for TableRowKind {
    fn collect_free(&self, free: &mut IdSet) {
        self.args.as_slice().collect_free(free);
        self.exp.collect_free(free);
        self.prems.as_slice().collect_free(free);
    }
}

// Hints alias EL hints and use their implementation.

// - Definitions

impl Free for ExternTypDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TypDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for VarDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for ExternRelDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for RelDef {
    fn collect_free(&self, free: &mut IdSet) {
        self.rule_groups.as_slice().collect_free(free);
        self.else_group.collect_free(free);
    }
}

impl Free for ExternDecDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for BuiltinDecDef {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for TableDecDef {
    fn collect_free(&self, free: &mut IdSet) {
        self.table_rows.as_slice().collect_free(free);
    }
}

impl Free for FuncDecDef {
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
