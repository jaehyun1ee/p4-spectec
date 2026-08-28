//! Syntax equality for structured-language data
//!
//! Ignores source regions;
//! compares relation hints and instruction identifiers

use crate::lang::traits::eq::SyntaxEq;

use super::ast::*;

// == Syntax equality

// - Holding case analysis

impl SyntaxEq for HoldCase {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                HoldCase::Both(block_hold_l, block_not_hold_l),
                HoldCase::Both(block_hold_r, block_not_hold_r),
            ) => {
                block_hold_l.syntax_eq(block_hold_r) && block_not_hold_l.syntax_eq(block_not_hold_r)
            }
            (HoldCase::Hold(block_l, dangle_l), HoldCase::Hold(block_r, dangle_r))
            | (HoldCase::NotHold(block_l, dangle_l), HoldCase::NotHold(block_r, dangle_r)) => {
                block_l.syntax_eq(block_r) && dangle_l == dangle_r
            }
            _ => false,
        }
    }
}

// - Case analysis

impl SyntaxEq for Case {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.guard.syntax_eq(&other.guard) && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for Guard {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Guard::Bool(value_l), Guard::Bool(value_r)) => value_l == value_r,
            (Guard::Cmp(op_l, typ_l, exp_l), Guard::Cmp(op_r, typ_r, exp_r)) => {
                op_l == op_r && typ_l == typ_r && exp_l.syntax_eq(exp_r)
            }
            (Guard::Sub(typ_l, _), Guard::Sub(typ_r, _)) => typ_l.syntax_eq(typ_r),
            (Guard::Match(pattern_l), Guard::Match(pattern_r)) => pattern_l.syntax_eq(pattern_r),
            (Guard::Mem(exp_l), Guard::Mem(exp_r)) => exp_l.syntax_eq(exp_r),
            _ => false,
        }
    }
}

// - Instructions

impl SyntaxEq for Block {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}

// - Relations

impl SyntaxEq for RelSignature {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.not_typ.syntax_eq(&other.not_typ) && self.input_hint.syntax_eq(&other.input_hint)
    }
}

// - Parameters

impl SyntaxEq for ParamKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ParamKind::Exp(typ_l, exp_l), ParamKind::Exp(typ_r, exp_r)) => {
                typ_l.syntax_eq(typ_r) && exp_l.syntax_eq(exp_r)
            }
            (
                ParamKind::Def(id_l, tparams_l, params_l, typ_l),
                ParamKind::Def(id_r, tparams_r, params_r, typ_r),
            ) => {
                id_l.syntax_eq(id_r)
                    && tparams_l.syntax_eq(tparams_r)
                    && params_l.syntax_eq(params_r)
                    && typ_l.syntax_eq(typ_r)
            }
            _ => false,
        }
    }
}

// - Instruction payloads

impl SyntaxEq for IfInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
            && self.iter_exps.syntax_eq(&other.iter_exps)
            && self.block.syntax_eq(&other.block)
            && self.dangle == other.dangle
    }
}

impl SyntaxEq for HoldInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.iter_exps.syntax_eq(&other.iter_exps)
            && self.hold_case.syntax_eq(&other.hold_case)
    }
}

impl SyntaxEq for CaseInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
            && self.cases.syntax_eq(&other.cases)
            && self.dangle == other.dangle
    }
}

impl SyntaxEq for GroupInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps.syntax_eq(&other.exps)
            && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for LetInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp_l.syntax_eq(&other.exp_l)
            && self.exp_r.syntax_eq(&other.exp_r)
            && self.iter_instrs.syntax_eq(&other.iter_instrs)
            && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for RuleInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.not_exp.syntax_eq(&other.not_exp)
            && self.input_hint.syntax_eq(&other.input_hint)
            && self.iter_instrs.syntax_eq(&other.iter_instrs)
            && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for ResultInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.rel_signature.syntax_eq(&other.rel_signature) && self.exps.syntax_eq(&other.exps)
    }
}

impl SyntaxEq for ReturnInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp)
    }
}

impl SyntaxEq for DebugInstr {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exp.syntax_eq(&other.exp) && self.instr.syntax_eq(&other.instr)
    }
}

impl SyntaxEq for InstrKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (InstrKind::If(instr_l), InstrKind::If(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Hold(instr_l), InstrKind::Hold(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Case(instr_l), InstrKind::Case(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Group(instr_l), InstrKind::Group(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Let(instr_l), InstrKind::Let(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Rule(instr_l), InstrKind::Rule(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Result(instr_l), InstrKind::Result(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Return(instr_l), InstrKind::Return(instr_r)) => instr_l.syntax_eq(instr_r),
            (InstrKind::Debug(instr_l), InstrKind::Debug(instr_r)) => instr_l.syntax_eq(instr_r),
            _ => false,
        }
    }
}

// - Relations and functions

impl SyntaxEq for ExternRel {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for Rel {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.rel_signature.syntax_eq(&other.rel_signature)
            && self.exps_input.syntax_eq(&other.exps_input)
            && self.block.syntax_eq(&other.block)
            && self.else_block.syntax_eq(&other.else_block)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for ExternFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for BuiltinFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TableRow {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.exps_input.syntax_eq(&other.exps_input)
            && self.exp.syntax_eq(&other.exp)
            && self.block.syntax_eq(&other.block)
    }
}

impl SyntaxEq for TableFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.table_rows.syntax_eq(&other.table_rows)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for DefinedFunc {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.params.syntax_eq(&other.params)
            && self.typ.syntax_eq(&other.typ)
            && self.block.syntax_eq(&other.block)
            && self.else_block.syntax_eq(&other.else_block)
            && self.hints.syntax_eq(&other.hints)
    }
}

// - Definitions

impl SyntaxEq for ExternTypDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id) && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for TypDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.tparams.syntax_eq(&other.tparams)
            && self.def_typ.syntax_eq(&other.def_typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for VarDef {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.id.syntax_eq(&other.id)
            && self.typ.syntax_eq(&other.typ)
            && self.hints.syntax_eq(&other.hints)
    }
}

impl SyntaxEq for DefKind {
    fn syntax_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DefKind::ExternTyp(def_l), DefKind::ExternTyp(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Typ(def_l), DefKind::Typ(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Var(def_l), DefKind::Var(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::ExternRel(def_l), DefKind::ExternRel(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::Rel(def_l), DefKind::Rel(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::ExternDec(def_l), DefKind::ExternDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::BuiltinDec(def_l), DefKind::BuiltinDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::TableDec(def_l), DefKind::TableDec(def_r)) => def_l.syntax_eq(def_r),
            (DefKind::FuncDec(def_l), DefKind::FuncDec(def_r)) => def_l.syntax_eq(def_r),
            _ => false,
        }
    }
}

// - Specifications

impl SyntaxEq for Spec {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.as_slice().syntax_eq(other.as_slice())
    }
}
