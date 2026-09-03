//! Free identifiers in structured-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Nodes through type parameters alias IL nodes and use their implementations.

// - Parameters

impl Free for ParamKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Exp(_, exp) => exp.free(),
            Self::Def(..) => IdSet::new(),
        }
    }
}

// Type arguments and arguments alias IL nodes and use their implementations.

// - Holding conditions

impl Free for HoldCase {
    fn free(&self) -> IdSet {
        match self {
            Self::Both(block_l, block_r) => block_l.free().union(block_r.free()),
            Self::Hold(block, _) | Self::NotHold(block, _) => block.free(),
        }
    }
}

// - Case analysis

impl Free for Case {
    fn free(&self) -> IdSet {
        self.guard.free().union(self.block.free())
    }
}

impl Free for Guard {
    fn free(&self) -> IdSet {
        match self {
            Self::Cmp(_, _, exp) | Self::Mem(exp) => exp.free(),
            Self::Bool(_) | Self::Sub(..) | Self::Match(_) => IdSet::new(),
        }
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
        self.not_exp.free()
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

// `ElseBlock` aliases `Block` and uses its implementation above.

// Iterator instructions alias IL iterator premises and use their implementation.

// Hints alias EL hints and use their implementation.

// - Relations

impl Free for RelSignature {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for ExternRel {
    fn free(&self) -> IdSet {
        self.exps_input.as_slice().free()
    }
}

impl Free for Rel {
    fn free(&self) -> IdSet {
        self.exps_input
            .as_slice()
            .free()
            .union(self.block.free())
            .union(self.else_block.free())
    }
}

// - Functions

impl Free for ExternFunc {
    fn free(&self) -> IdSet {
        self.params.as_slice().free()
    }
}

impl Free for BuiltinFunc {
    fn free(&self) -> IdSet {
        self.params.as_slice().free()
    }
}

impl Free for TableRow {
    fn free(&self) -> IdSet {
        self.exps_input
            .as_slice()
            .free()
            .union(self.exp.free())
            .union(self.block.free())
    }
}

impl Free for TableFunc {
    fn free(&self) -> IdSet {
        self.params
            .as_slice()
            .free()
            .union(self.table_rows.as_slice().free())
    }
}

impl Free for DefinedFunc {
    fn free(&self) -> IdSet {
        self.params
            .as_slice()
            .free()
            .union(self.block.free())
            .union(self.else_block.free())
    }
}

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
