//! Collect expression identifiers, including patterns and both Hold paths
use super::ast::*;
use crate::lang::{common::ds::set::IdSet, traits::free::Free};

impl Free for Case {
    fn free(&self) -> IdSet {
        self.guard.free().union(self.block.free())
    }
}

// - Instructions

impl Free for InstrKind {
    fn free(&self) -> IdSet {
        match self {
            Self::If(instr) => instr.free(),
            Self::Hold(instr) => instr.free(),
            Self::Case(instr) => instr.free(),
            Self::Group(instr) => instr.free(),
            Self::Let(instr) => instr.free(),
            Self::Rule(instr) => instr.free(),
            Self::Result(instr) => instr.free(),
            Self::Return(instr) => instr.free(),
            Self::Debug(instr) => instr.free(),
        }
    }
}

impl Free for IfInstr {
    fn free(&self) -> IdSet {
        self.exp.free().union(self.block.free())
    }
}

impl Free for HoldInstr {
    fn free(&self) -> IdSet {
        self.not_exp
            .free()
            .union(self.block_hold.free())
            .union(self.block_not_hold.free())
    }
}

impl Free for CaseInstr {
    fn free(&self) -> IdSet {
        self.exp.free().union(self.cases.as_slice().free())
    }
}

impl Free for GroupInstr {
    fn free(&self) -> IdSet {
        self.exps.as_slice().free().union(self.block.free())
    }
}

impl Free for LetInstr {
    fn free(&self) -> IdSet {
        self.exp_l
            .free()
            .union(self.exp_r.free())
            .union(self.block.free())
    }
}

impl Free for RuleInstr {
    fn free(&self) -> IdSet {
        self.not_exp.free().union(self.block.free())
    }
}

impl Free for ResultInstr {
    fn free(&self) -> IdSet {
        self.exps.as_slice().free()
    }
}

impl Free for ReturnInstr {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for DebugInstr {
    fn free(&self) -> IdSet {
        self.exp.free().union(self.instr.free())
    }
}

impl Free for Block {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}

// `ElseBlock` aliases `Block` and uses its implementation above
