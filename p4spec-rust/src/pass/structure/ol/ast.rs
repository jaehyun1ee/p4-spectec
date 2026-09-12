//! Intermediate structured instructions used during optimization
pub use crate::lang::sl::ast::{Exp, ExpIter, Guard, Id, InstrIter, NotExp, RelSignature};
use crate::lang::{common::source::Phrase, hints::input::InputHint};
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub guard: Guard,
    pub block: Block,
}

// Instructions

pub type Instr = Phrase<InstrKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum InstrKind {
    If(IfInstr),
    Hold(HoldInstr),
    Case(CaseInstr),
    Group(GroupInstr),
    Let(LetInstr),
    Rule(RuleInstr),
    Result(ResultInstr),
    Return(ReturnInstr),
    Debug(DebugInstr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfInstr {
    pub exp: Exp,
    pub iter_exps: Vec<ExpIter>,
    pub block: Block,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HoldInstr {
    pub id: Id,
    pub not_exp: NotExp,
    pub iter_exps: Vec<ExpIter>,
    pub block_hold: Block,
    pub block_not_hold: Block,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CaseInstr {
    pub exp: Exp,
    pub cases: Vec<Case>,
    pub total: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct GroupInstr {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps: Vec<Exp>,
    pub block: Block,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LetInstr {
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub iter_instrs: Vec<InstrIter>,
    pub block: Block,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuleInstr {
    pub id: Id,
    pub not_exp: NotExp,
    pub input_hint: InputHint,
    pub iter_instrs: Vec<InstrIter>,
    pub block: Block,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResultInstr {
    pub rel_signature: RelSignature,
    pub exps: Vec<Exp>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnInstr {
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugInstr {
    pub exp: Exp,
    pub instr: Box<Instr>,
}

pub type Block = Vec<Instr>;
pub type ElseBlock = Block;
