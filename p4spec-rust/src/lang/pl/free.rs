//! Free identifiers in prose-language data

use crate::lang::{common::ds::set::IdSet, traits::free::Free};

use super::ast::*;

// == Free identifiers

// Nodes through subtype checks alias SL or IL nodes and use their implementations.

// - Expressions

impl Free for ExpKind {
    fn collect_free(&self, free: &mut IdSet) {
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
            | Self::Iter(exp, _) => exp.collect_free(free),
            Self::Bin(_, _, exp_l, exp_r)
            | Self::Cmp(_, _, exp_l, exp_r)
            | Self::Cons(exp_l, exp_r)
            | Self::Cat(exp_l, exp_r)
            | Self::Mem(exp_l, exp_r)
            | Self::Idx(exp_l, exp_r) => {
                exp_l.collect_free(free);
                exp_r.collect_free(free);
            }
            Self::Tuple(exps) | Self::List(exps) => exps.as_slice().collect_free(free),
            Self::Case(not_exp) => not_exp.collect_free(free),
            Self::Str(fields) => {
                for (_, exp) in fields {
                    exp.collect_free(free);
                }
            }
            Self::Opt(exp) => exp.collect_free(free),
            Self::Slice(exp_b, exp_i, exp_n) => {
                exp_b.collect_free(free);
                exp_i.collect_free(free);
                exp_n.collect_free(free);
            }
            Self::Upd(exp_b, path, exp_f) => {
                exp_b.collect_free(free);
                path.collect_free(free);
                exp_f.collect_free(free);
            }
            Self::Call(_, _, args) => args.as_slice().collect_free(free),
        }
    }
}

// Iterator expressions and patterns alias IL nodes and use their implementations.

// - Paths

impl Free for PathKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Root => {}
            Self::Idx(path, exp_i) => {
                path.collect_free(free);
                exp_i.collect_free(free);
            }
            Self::Slice(path, exp_i, exp_n) => {
                path.collect_free(free);
                exp_i.collect_free(free);
                exp_n.collect_free(free);
            }
            Self::Dot(path, _) => path.collect_free(free),
        }
    }
}

// Type parameters alias IL nodes and use their implementations.

// - Parameters

impl Free for ParamKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Exp(_, exp) => exp.collect_free(free),
            Self::Def(..) => {}
        }
    }
}

// Type arguments alias IL nodes and use their implementations.

// - Arguments

impl Free for ArgKind {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Exp(exp) => exp.collect_free(free),
            Self::Def(_) => {}
        }
    }
}

// Dangling markers alias SL markers and use their implementation.

// - Holding conditions

impl<Tier: Free> Free for HoldCase<Tier> {
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

impl<Tier: Free> Free for Case<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.guard.collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for Guard {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Cmp(_, _, exp) | Self::Mem(exp) => exp.collect_free(free),
            Self::CheckLetSub(_, _, exp) | Self::CheckLetMatch(_, exp) => {
                exp.collect_free(free);
            }
            Self::Bool(_) | Self::Sub(..) | Self::Match(_) => {}
        }
    }
}

// - Instructions

impl Free for Fallthrough {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl Free for InstrNote {
    fn collect_free(&self, _free: &mut IdSet) {}
}

impl<Tier: Free> Free for Block<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.as_slice().collect_free(free);
    }
}

impl<Tier: Free> Free for InstrKind<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::If(instr) => instr.collect_free(free),
            Self::Hold(instr) => instr.collect_free(free),
            Self::Case(instr) => instr.collect_free(free),
            Self::Let(instr) => instr.collect_free(free),
            Self::Debug(instr) => instr.collect_free(free),
            Self::Destruct(instr) => instr.collect_free(free),
            Self::CheckLetSub(instr) => instr.collect_free(free),
            Self::CheckLetMatch(instr) => instr.collect_free(free),
            Self::OptionGet(instr) => instr.collect_free(free),
            Self::Tier(instr) => instr.collect_free(free),
        }
    }
}

impl<Tier: Free> Free for IfInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
        self.block.collect_free(free);
    }
}

impl<Tier: Free> Free for HoldInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
    }
}

impl<Tier: Free> Free for CaseInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
        self.cases.as_slice().collect_free(free);
    }
}

impl Free for LetInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp_l.collect_free(free);
        self.exp_r.collect_free(free);
    }
}

impl Free for DebugInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for DestructInstr {
    fn collect_free(&self, free: &mut IdSet) {
        for (_, exp) in &self.bindings {
            exp.collect_free(free);
        }
        self.exp.collect_free(free);
    }
}

impl<Tier: Free> Free for CheckLetSubInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp_l.collect_free(free);
        self.exp_r.collect_free(free);
        self.block.collect_free(free);
    }
}

impl<Tier: Free> Free for CheckLetMatchInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp_l.collect_free(free);
        self.exp_r.collect_free(free);
        self.block.collect_free(free);
    }
}

impl<Tier: Free> Free for OptionGetInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp_l.collect_free(free);
        self.exp_r.collect_free(free);
        self.block.collect_free(free);
    }
}

impl<Tier: Free> Free for TierInstr<Tier> {
    fn collect_free(&self, free: &mut IdSet) {
        self.tier.collect_free(free);
    }
}

// Iterator instructions and relation signatures alias SL nodes and use their implementations.

// - Group-body tier

impl Free for InstrGroup {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Result(instr) => instr.collect_free(free),
            Self::Return(instr) => instr.collect_free(free),
            Self::Rule(instr) => instr.collect_free(free),
            Self::Backtrack(instr) => instr.collect_free(free),
        }
    }
}

impl Free for ResultGroupInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_output.as_slice().collect_free(free);
    }
}

impl Free for ReturnGroupInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exp.collect_free(free);
    }
}

impl Free for RuleGroupInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.not_exp.collect_free(free);
    }
}

impl Free for BacktrackGroupInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.blocks.as_slice().collect_free(free);
    }
}

// `BlockGroup` uses the generic block implementation above.

// - Dispatch tier

impl Free for InstrDispatch {
    fn collect_free(&self, free: &mut IdSet) {
        match self {
            Self::Group(instr) => instr.collect_free(free),
            Self::Route(instr) => instr.collect_free(free),
        }
    }
}

impl Free for GroupDispatchInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_input.as_slice().collect_free(free);
        self.block.collect_free(free);
    }
}

impl Free for RouteDispatchInstr {
    fn collect_free(&self, free: &mut IdSet) {
        self.blocks.as_slice().collect_free(free);
    }
}

// `BlockDispatch` uses the generic block implementation above.

// - Relations and functions

impl Free for ExternRel {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_input.as_slice().collect_free(free);
    }
}

impl Free for Rel {
    fn collect_free(&self, free: &mut IdSet) {
        self.exps_input.as_slice().collect_free(free);
        self.block.collect_free(free);
        self.block_else_opt.collect_free(free);
    }
}

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
        self.rows.as_slice().collect_free(free);
    }
}

impl Free for DefinedFunc {
    fn collect_free(&self, free: &mut IdSet) {
        self.params.as_slice().collect_free(free);
        self.block.collect_free(free);
        self.block_else_opt.collect_free(free);
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
