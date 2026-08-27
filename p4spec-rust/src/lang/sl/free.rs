//! Free identifiers in structured-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Nodes through type parameters alias IL nodes and use their implementations.

// - Parameters

impl Free for ParamKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Exp(_, exp) => exp.collect_free(free),
            Self::Def(..) => {}
        }
    }
}

// Type arguments and arguments alias IL nodes and use their implementations.

// - Holding conditions

impl Free for HoldCase {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Both(block_l, block_r) => {
                block_l.collect_free(free);
                block_r.collect_free(free);
            }
            Self::Hold(block, _) | Self::NotHold(block, _) => block.collect_free(free),
        }
    }
}

// - Case analysis

impl Free for Case {
    fn collect_free(&self, free: &mut IdSet) {
        self.guard.collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for Guard {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Cmp(_, _, exp) | Self::Mem(exp) => exp.collect_free(free),
            Self::Bool(_) | Self::Sub(..) | Self::Match(_) => {}
        }
    }
}

// - Instructions

impl Free for Iid {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for InstrKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::If(instr) => instr.collect_free(free),
            Self::Hold(instr) => instr.collect_free(free),
            Self::Case(instr) => instr.collect_free(free),
            Self::Group(instr) => instr.collect_free(free),
            Self::Let(instr) => instr.collect_free(free),
            Self::Rule(instr) => instr.collect_free(free),
            Self::Result(instr) => instr.collect_free(free),
            Self::Return(instr) => instr.collect_free(free),
            Self::Debug(instr) => instr.collect_free(free),
        }
    }
}

impl Free for IfInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for HoldInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
    }
}

impl Free for CaseInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
        self.cases.as_slice().collect_free(free);
    }
}

impl Free for GroupInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps.as_slice().collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for LetInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp_l.collect_free(free);
        self.exp_r.collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for RuleInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for ResultInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps.as_slice().collect_free(free);
    }
}

impl Free for ReturnInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for DebugInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
        self.instr.collect_free(free);
    }
}

impl Free for Block {
    fn collect_free(&self, free: &mut IdSet) {
        self.as_slice().collect_free(free);
    }
}

// `ElseBlock` aliases `Block` and uses its implementation above.

// Iterator instructions alias IL iterator premises and use their implementation.

// Hints alias EL hints and use their implementation.

// - Relations

impl Free for RelSignature {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for ExternRel {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_input.as_slice().collect_free(free);
    }
}

impl Free for Rel {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_input.as_slice().collect_free(free);
        self.block.collect_free(free);
        self.else_block.collect_free(free);
    }
}

// - Functions

impl Free for ExternFunc {
    fn collect_free(&self, free: &mut IdSet) {
        self.params.as_slice().collect_free(free);
    }
}

impl Free for BuiltinFunc {
    fn collect_free(&self, free: &mut IdSet) {
        self.params.as_slice().collect_free(free);
    }
}

impl Free for TableRow {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_input.as_slice().collect_free(free);
        self.exp.collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for TableFunc {
    fn collect_free(&self, free: &mut IdSet) {
        self.params.as_slice().collect_free(free);
        self.table_rows.as_slice().collect_free(free);
    }
}

impl Free for DefinedFunc {
    fn collect_free(&self, free: &mut IdSet) {
        self.params.as_slice().collect_free(free);
        self.block.collect_free(free);
        self.else_block.collect_free(free);
    }
}

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
