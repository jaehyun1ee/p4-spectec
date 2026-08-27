//! Free identifiers in prose-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Nodes through subtype checks alias SL or IL nodes and use their implementations.

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

// Iterator expressions and patterns alias IL nodes and use their implementations.

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

// Type parameters alias IL nodes and use their implementations.

// - Parameters

impl Free for ParamKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Exp(_, exp) => exp.free(),
            Self::Def(..) => IdSet::new(),
        }
    }
}

// Type arguments alias IL nodes and use their implementations.

// - Arguments

impl Free for ArgKind {
    fn free(&self) -> IdSet {
        match self {
            Self::Exp(exp) => exp.free(),
            Self::Def(_) => IdSet::new(),
        }
    }
}

// Dangling markers alias SL markers and use their implementation.

// - Holding conditions

impl<Tier: Free> Free for HoldCase<Tier> {
    fn free(&self) -> IdSet {
        match self {
            Self::Both(block_l, block_r) => block_l.free().union(block_r.free()),
            Self::Hold(block, _) | Self::NotHold(block, _) => block.free(),
        }
    }
}

// - Case analysis

impl<Tier: Free> Free for Case<Tier> {
    fn free(&self) -> IdSet {
        self.guard.free().union(self.block.free())
    }
}

impl Free for Guard {
    fn free(&self) -> IdSet {
        match self {
            Self::Cmp(_, _, exp) | Self::Mem(exp) => exp.free(),
            Self::CheckLetSub(_, _, exp) | Self::CheckLetMatch(_, exp) => exp.free(),
            Self::Bool(_) | Self::Sub(..) | Self::Match(_) => IdSet::new(),
        }
    }
}

// - Instructions

impl Free for Fallthrough {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl Free for InstrNote {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

impl<Tier: Free> Free for Block<Tier> {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}

impl<Tier: Free> Free for InstrKind<Tier> {
    fn free(&self) -> IdSet {
        match self {
            Self::If(instr) => instr.free(),
            Self::Hold(instr) => instr.free(),
            Self::Case(instr) => instr.free(),
            Self::Let(instr) => instr.free(),
            Self::Debug(instr) => instr.free(),
            Self::Destruct(instr) => instr.free(),
            Self::CheckLetSub(instr) => instr.free(),
            Self::CheckLetMatch(instr) => instr.free(),
            Self::OptionGet(instr) => instr.free(),
            Self::Tier(instr) => instr.free(),
        }
    }
}

impl<Tier: Free> Free for IfInstr<Tier> {
    fn free(&self) -> IdSet {
        self.exp.free().union(self.block.free())
    }
}

impl<Tier: Free> Free for HoldInstr<Tier> {
    fn free(&self) -> IdSet {
        self.not_exp.free()
    }
}

impl<Tier: Free> Free for CaseInstr<Tier> {
    fn free(&self) -> IdSet {
        self.exp.free().union(self.cases.as_slice().free())
    }
}

impl Free for LetInstr {
    fn free(&self) -> IdSet {
        self.exp_l.free().union(self.exp_r.free())
    }
}

impl Free for DebugInstr {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for DestructInstr {
    fn free(&self) -> IdSet {
        self.bindings
            .iter()
            .fold(IdSet::new(), |free, (_, exp)| free.union(exp.free()))
            .union(self.exp.free())
    }
}

impl<Tier: Free> Free for CheckLetSubInstr<Tier> {
    fn free(&self) -> IdSet {
        self.exp_l
            .free()
            .union(self.exp_r.free())
            .union(self.block.free())
    }
}

impl<Tier: Free> Free for CheckLetMatchInstr<Tier> {
    fn free(&self) -> IdSet {
        self.exp_l
            .free()
            .union(self.exp_r.free())
            .union(self.block.free())
    }
}

impl<Tier: Free> Free for OptionGetInstr<Tier> {
    fn free(&self) -> IdSet {
        self.exp_l
            .free()
            .union(self.exp_r.free())
            .union(self.block.free())
    }
}

impl<Tier: Free> Free for TierInstr<Tier> {
    fn free(&self) -> IdSet {
        self.tier.free()
    }
}

// Iterator instructions and relation signatures alias SL nodes and use their implementations.

// - Group-body tier

impl Free for InstrGroup {
    fn free(&self) -> IdSet {
        match self {
            Self::Result(instr) => instr.free(),
            Self::Return(instr) => instr.free(),
            Self::Rule(instr) => instr.free(),
            Self::Backtrack(instr) => instr.free(),
        }
    }
}

impl Free for ResultGroupInstr {
    fn free(&self) -> IdSet {
        self.exps_output.as_slice().free()
    }
}

impl Free for ReturnGroupInstr {
    fn free(&self) -> IdSet {
        self.exp.free()
    }
}

impl Free for RuleGroupInstr {
    fn free(&self) -> IdSet {
        self.not_exp.free()
    }
}

impl Free for BacktrackGroupInstr {
    fn free(&self) -> IdSet {
        self.blocks.as_slice().free()
    }
}

// `BlockGroup` uses the generic block implementation above.

// - Dispatch tier

impl Free for InstrDispatch {
    fn free(&self) -> IdSet {
        match self {
            Self::Group(instr) => instr.free(),
            Self::Route(instr) => instr.free(),
        }
    }
}

impl Free for GroupDispatchInstr {
    fn free(&self) -> IdSet {
        self.exps_input.as_slice().free().union(self.block.free())
    }
}

impl Free for RouteDispatchInstr {
    fn free(&self) -> IdSet {
        self.blocks.as_slice().free()
    }
}

// `BlockDispatch` uses the generic block implementation above.

// - Relations and functions

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
            .union(self.block_else_opt.free())
    }
}

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
            .union(self.rows.as_slice().free())
    }
}

impl Free for DefinedFunc {
    fn free(&self) -> IdSet {
        self.params
            .as_slice()
            .free()
            .union(self.block.free())
            .union(self.block_else_opt.free())
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

impl Free for Spec {
    fn free(&self) -> IdSet {
        self.as_slice().free()
    }
}
