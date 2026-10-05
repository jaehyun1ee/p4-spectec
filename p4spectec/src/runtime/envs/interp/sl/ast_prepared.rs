//! Slot instantiation of shared SL syntax
//!
//! The `Prepare` impls rebuild each node with slots in place of names,
//! reserving slots in the callable's `FrameLayout` as they go.
//! Extern and builtin definitions prepare their parameters only.

use crate::lang::{hints::input, sl::ast as source, traits::eq::SyntaxEq};

pub use crate::lang::sl::ast::{DefinedTyp, ExternTyp, RelSignature, TypDef, VarDef};

use crate::interp::shared::prepare::{Prepare, PrepareContext};

pub use crate::interp::shared::prepare::ast::*;

// == Prepared syntax

// - Parameters

pub type Param = source::Param<Prepared>;
pub type ParamKind = source::ParamKind<Prepared>;

// - Dangling

pub type Dangle = source::Dangle;

// - Holding conditions

pub type HoldCase = source::HoldCase<Prepared>;

// - Case analysis

pub type Guard = source::Guard<Prepared>;
pub type Case = source::Case<Prepared>;

// - Instructions

pub type Instr = source::Instr<Prepared>;
pub type InstrKind = source::InstrKind<Prepared>;
pub type IfInstr = source::IfInstr<Prepared>;
pub type HoldInstr = source::HoldInstr<Prepared>;
pub type CaseInstr = source::CaseInstr<Prepared>;
pub type GroupInstr = source::GroupInstr<Prepared>;
pub type LetInstr = source::LetInstr<Prepared>;
pub type RuleInstr = source::RuleInstr<Prepared>;
pub type ResultInstr = source::ResultInstr<Prepared>;
pub type ReturnInstr = source::ReturnInstr<Prepared>;
pub type DebugInstr = source::DebugInstr<Prepared>;
pub type InstrIter = PremIter;

// - Blocks

pub type Block = source::Block<Prepared>;
pub type ElseBlock = source::ElseBlock<Prepared>;

// - Table rows

pub type TableRow = source::TableRow<Prepared>;

// - Relation definitions

pub type RelDef = source::RelDef<Prepared>;
pub type ExternRel = source::ExternRel<Prepared>;
pub type DefinedRel = source::DefinedRel<Prepared>;

// - Meta-function definitions

pub type MetaFuncDef = source::MetaFuncDef<Prepared>;
pub type ExternFunc = source::ExternFunc<Prepared>;
pub type BuiltinFunc = source::BuiltinFunc<Prepared>;
pub type TableFunc = source::TableFunc<Prepared>;
pub type DefinedFunc = source::DefinedFunc<Prepared>;

// - Definitions

pub type Def = source::Def<Prepared>;
pub type DefKind = source::DefKind<Prepared>;
pub type Spec = Vec<Def>;

// == Preparation traversal

// - Parameters

impl Prepare for source::ParamKind {
    type Output = ParamKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::ParamKind::Exp(typ_inner, exp_inner) => {
                ParamKind::Exp(typ_inner, exp_inner.prepare(ctx))
            }
            source::ParamKind::Def(id_inner, tparams_inner, params_inner, typ_inner) => {
                ParamKind::Def(id_inner, tparams_inner, params_inner.prepare(ctx), typ_inner)
            }
        }
    }
}

// - Instructions

impl Prepare for source::InstrKind {
    type Output = InstrKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::InstrKind::If(instr_inner) => InstrKind::If(instr_inner.prepare(ctx)),
            source::InstrKind::Hold(instr_inner) => InstrKind::Hold(instr_inner.prepare(ctx)),
            source::InstrKind::Case(instr_inner) => InstrKind::Case(instr_inner.prepare(ctx)),
            source::InstrKind::Group(instr_inner) => InstrKind::Group(instr_inner.prepare(ctx)),
            source::InstrKind::Let(instr_inner) => InstrKind::Let(instr_inner.prepare(ctx)),
            source::InstrKind::Rule(instr_inner) => InstrKind::Rule(instr_inner.prepare(ctx)),
            source::InstrKind::Result(instr_inner) => InstrKind::Result(instr_inner.prepare(ctx)),
            source::InstrKind::Return(instr_inner) => InstrKind::Return(instr_inner.prepare(ctx)),
            source::InstrKind::Debug(instr_inner) => InstrKind::Debug(instr_inner.prepare(ctx)),
        }
    }
}

// - If instruction

impl Prepare for source::IfInstr {
    type Output = IfInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        IfInstr {
            exp: self.exp.prepare(ctx),
            iter_exps: self.iter_exps.prepare(ctx),
            block: self.block.prepare(ctx),
            dangle: self.dangle,
        }
    }
}

// - Hold instruction

impl Prepare for source::HoldInstr {
    type Output = HoldInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        HoldInstr {
            id: self.id,
            not_exp: self.not_exp.prepare(ctx),
            iter_exps: self.iter_exps.prepare(ctx),
            hold_case: self.hold_case.prepare(ctx),
        }
    }
}

// - Case instruction

impl Prepare for source::CaseInstr {
    type Output = CaseInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        CaseInstr {
            exp: self.exp.prepare(ctx),
            cases: self.cases.prepare(ctx),
            dangle: self.dangle,
        }
    }
}

// - Group instruction

impl Prepare for source::GroupInstr {
    type Output = GroupInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        GroupInstr {
            id: self.id,
            rel_signature: self.rel_signature,
            exps: self.exps.prepare(ctx),
            block: self.block.prepare(ctx),
        }
    }
}

// - Let instruction

impl Prepare for source::LetInstr {
    type Output = LetInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        LetInstr {
            exp_l: self.exp_l.prepare(ctx),
            exp_r: self.exp_r.prepare(ctx),
            iter_instrs: self.iter_instrs.prepare(ctx),
            block: self.block.prepare(ctx),
        }
    }
}

// - Rule instruction

impl Prepare for source::RuleInstr {
    type Output = RuleInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        let returns_outputs = returns_outputs(&self);
        RuleInstr {
            id: self.id,
            not_exp: self.not_exp.prepare(ctx),
            input_hint: self.input_hint,
            iter_instrs: self.iter_instrs.prepare(ctx),
            block: self.block.prepare(ctx),
            returns_outputs,
        }
    }
}

/// Whether the rule's block only returns the call's outputs unchanged.
///
/// Such a call in tail position becomes a tail call.
fn returns_outputs(instr: &source::RuleInstr) -> bool {
    let exps = instr.not_exp.args().iter().collect();
    let (_, exps_output) =
        input::split(&instr.input_hint, exps).expect("input hint must fit relation");
    let [instr_result] = instr.block.as_slice() else { return false };
    let source::InstrKind::Result(instr_result) = &instr_result.node else { return false };
    instr.iter_instrs.is_empty()
        && exps_output.len() == instr_result.exps.len()
        && exps_output
            .iter()
            .zip(&instr_result.exps)
            .all(|(exp_l, exp_r)| exp_l.syntax_eq(exp_r))
}

// - Result instruction

impl Prepare for source::ResultInstr {
    type Output = ResultInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ResultInstr { rel_signature: self.rel_signature, exps: self.exps.prepare(ctx) }
    }
}

// - Return instruction

impl Prepare for source::ReturnInstr {
    type Output = ReturnInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ReturnInstr { exp: self.exp.prepare(ctx) }
    }
}

// - Debug instruction

impl Prepare for source::DebugInstr {
    type Output = DebugInstr;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        DebugInstr { exp: self.exp.prepare(ctx), instr: self.instr.prepare(ctx) }
    }
}

// - Holding conditions

impl Prepare for source::HoldCase {
    type Output = HoldCase;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::HoldCase::Both(block_hold, block_not_hold) => {
                HoldCase::Both(block_hold.prepare(ctx), block_not_hold.prepare(ctx))
            }
            source::HoldCase::Hold(block_inner, dangle_inner) => {
                HoldCase::Hold(block_inner.prepare(ctx), dangle_inner)
            }
            source::HoldCase::NotHold(block_inner, dangle_inner) => {
                HoldCase::NotHold(block_inner.prepare(ctx), dangle_inner)
            }
        }
    }
}

// - Case analysis

impl Prepare for source::Guard {
    type Output = Guard;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::Guard::Bool(value_inner) => Guard::Bool(value_inner),
            source::Guard::Cmp(op_inner, typ_op_inner, exp_inner) => {
                Guard::Cmp(op_inner, typ_op_inner, exp_inner.prepare(ctx))
            }
            source::Guard::Sub(typ_inner, subcheck_inner) => {
                Guard::Sub(typ_inner, subcheck_inner.prepare(ctx))
            }
            source::Guard::Match(pattern_inner) => Guard::Match(pattern_inner.prepare(ctx)),
            source::Guard::Mem(exp_inner) => Guard::Mem(exp_inner.prepare(ctx)),
        }
    }
}

impl Prepare for source::Case {
    type Output = Case;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        Case { guard: self.guard.prepare(ctx), block: self.block.prepare(ctx) }
    }
}

// - Table rows

impl Prepare for source::TableRow {
    type Output = TableRow;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        TableRow {
            exps_input: self.exps_input.prepare(ctx),
            exp: self.exp.prepare(ctx),
            block: self.block.prepare(ctx),
        }
    }
}

// == Relation definitions

// - Relation definition

impl Prepare for source::RelDef {
    type Output = RelDef;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::RelDef::Extern(rel_inner) => RelDef::Extern(rel_inner.prepare(ctx)),
            source::RelDef::Defined(rel_inner) => RelDef::Defined(rel_inner.prepare(ctx)),
        }
    }
}

// - External relation definition

impl Prepare for source::ExternRel {
    type Output = ExternRel;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ExternRel {
            id: self.id,
            rel_signature: self.rel_signature,
            exps_input: self.exps_input.prepare(ctx),
            hints: self.hints,
        }
    }
}

// - Defined relation definition

impl Prepare for source::DefinedRel {
    type Output = DefinedRel;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        DefinedRel {
            id: self.id,
            rel_signature: self.rel_signature,
            exps_input: self.exps_input.prepare(ctx),
            block: self.block.prepare(ctx),
            block_else: self.block_else.prepare(ctx),
            hints: self.hints,
        }
    }
}

// == Meta-function definitions

// - Meta-function definition

impl Prepare for source::MetaFuncDef {
    type Output = MetaFuncDef;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::MetaFuncDef::Extern(func_inner) => MetaFuncDef::Extern(func_inner.prepare(ctx)),
            source::MetaFuncDef::Builtin(func_inner) => {
                MetaFuncDef::Builtin(func_inner.prepare(ctx))
            }
            source::MetaFuncDef::Table(func_inner) => MetaFuncDef::Table(func_inner.prepare(ctx)),
            source::MetaFuncDef::Defined(func_inner) => {
                MetaFuncDef::Defined(func_inner.prepare(ctx))
            }
        }
    }
}

// - External function definition

impl Prepare for source::ExternFunc {
    type Output = ExternFunc;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        ExternFunc {
            id: self.id,
            tparams: self.tparams,
            params: self.params.prepare(ctx),
            typ: self.typ,
            hints: self.hints,
        }
    }
}

// - Builtin function definition

impl Prepare for source::BuiltinFunc {
    type Output = BuiltinFunc;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        BuiltinFunc {
            id: self.id,
            tparams: self.tparams,
            params: self.params.prepare(ctx),
            typ: self.typ,
            hints: self.hints,
        }
    }
}

// - Table function definition

impl Prepare for source::TableFunc {
    type Output = TableFunc;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        TableFunc {
            id: self.id,
            params: self.params.prepare(ctx),
            typ: self.typ,
            table_rows: self.table_rows.prepare(ctx),
            hints: self.hints,
        }
    }
}

// - Defined function definition

impl Prepare for source::DefinedFunc {
    type Output = DefinedFunc;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        DefinedFunc {
            id: self.id,
            tparams: self.tparams,
            params: self.params.prepare(ctx),
            typ: self.typ,
            block: self.block.prepare(ctx),
            block_else: self.block_else.prepare(ctx),
            hints: self.hints,
        }
    }
}

// == Definitions

impl Prepare for source::DefKind {
    type Output = DefKind;

    fn prepare(self, ctx: &mut PrepareContext<'_>) -> Self::Output {
        match self {
            source::DefKind::Typ(typdef) => DefKind::Typ(typdef),
            source::DefKind::Var(def_var) => DefKind::Var(def_var),
            source::DefKind::Rel(rel_inner) => DefKind::Rel(rel_inner.prepare(ctx)),
            source::DefKind::MetaFunc(func_inner) => DefKind::MetaFunc(func_inner.prepare(ctx)),
        }
    }
}
