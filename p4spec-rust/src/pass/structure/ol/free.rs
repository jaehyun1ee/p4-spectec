//! Free identifiers in optimization-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// - Instructions

impl Free for InstrKind {
    fn free_into(&self, free: &mut IdSet) {
        match self {
            Self::If(instr) => instr.free_into(free),
            Self::Hold(instr) => instr.free_into(free),
            Self::Case(instr) => instr.free_into(free),
            Self::Group(instr) => instr.free_into(free),
            Self::Let(instr) => instr.free_into(free),
            Self::Rule(instr) => instr.free_into(free),
            Self::Result(instr) => instr.free_into(free),
            Self::Return(instr) => instr.free_into(free),
            Self::Debug(instr) => instr.free_into(free),
        }
    }
}

impl Free for IfInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
        self.block.free_into(free);
    }
}

impl Free for HoldInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.not_exp.free_into(free);
        self.block_hold.free_into(free);
        self.block_not_hold.free_into(free);
    }
}

impl Free for CaseInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
        self.cases.as_slice().free_into(free);
    }
}

impl Free for GroupInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exps.as_slice().free_into(free);
        self.block.free_into(free);
    }
}

impl Free for LetInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exp_l.free_into(free);
        self.exp_r.free_into(free);
        self.block.free_into(free);
    }
}

impl Free for RuleInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.not_exp.free_into(free);
        self.block.free_into(free);
    }
}

impl Free for ResultInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exps.as_slice().free_into(free);
    }
}

impl Free for ReturnInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
    }
}

impl Free for DebugInstr {
    fn free_into(&self, free: &mut IdSet) {
        self.exp.free_into(free);
        self.instr.free_into(free);
    }
}

// - Case analysis

impl Free for Case {
    fn free_into(&self, free: &mut IdSet) {
        self.guard.free_into(free);
        self.block.free_into(free);
    }
}

// - Blocks

impl Free for Block {
    fn free_into(&self, free: &mut IdSet) {
        self.as_slice().free_into(free);
    }
}
