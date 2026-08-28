//! Structured language model

use crate::lang::{
    common::{
        noted::Noted,
        source::{Span, Spanned},
    },
    el,
    hints::input::InputHint,
    il,
};

// Numbers

pub type Num = il::ast::Num;

// Texts

pub type Text = il::ast::Text;

// Identifiers

pub type Id = il::ast::Id;
pub type IdKind = il::ast::IdKind;

// Atoms

pub type Atom = il::ast::Atom;

// Mixfix operators

pub type Mixop = il::ast::Mixop;

// Iterators

pub type Iter = il::ast::Iter;

// Variables

pub type Var = il::ast::Var;

// Types

pub type Typ = il::ast::Typ;
pub type TypKind = il::ast::TypKind;

pub type NotTyp = il::ast::NotTyp;
pub type NotTypKind = il::ast::NotTypKind;

pub type DefTyp = il::ast::DefTyp;
pub type DefTypKind = il::ast::DefTypKind;

pub type TypField = il::ast::TypField;
pub type TypCase = il::ast::TypCase;

// Values

pub type Value = il::ast::Value;
pub type ValueKind = il::ast::ValueKind;

pub type ValueField = il::ast::ValueField;
pub type ValueCase = il::ast::ValueCase;

// Operators

pub type NumOp = il::ast::NumOp;
pub type UnOp = il::ast::UnOp;
pub type BinOp = il::ast::BinOp;
pub type CmpOp = il::ast::CmpOp;
pub type OpTyp = il::ast::OpTyp;

// Subtype checks

pub type Subcheck = il::ast::Subcheck;

// Expressions

pub type Exp = il::ast::Exp;
pub type ExpKind = il::ast::ExpKind;

pub type NotExp = il::ast::NotExp;
pub type IterExp = il::ast::IterExp;

// Patterns

pub type Pattern = il::ast::Pattern;

// Path

pub type Path = il::ast::Path;
pub type PathKind = il::ast::PathKind;

// Type parameters

pub type TParam = il::ast::TParam;
pub type TParamKind = il::ast::TParamKind;

// Parameters

pub type Param = Spanned<ParamKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    Exp(Typ, Box<Exp>),
    Def(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type arguments

pub type Targ = il::ast::Targ;
pub type TargKind = il::ast::TargKind;

// Arguments

pub type Arg = il::ast::Arg;
pub type ArgKind = il::ast::ArgKind;

// Dangling

pub type Dangle = bool;

// Holding conditions

#[derive(Clone, Debug, PartialEq)]
pub enum HoldCase {
    Both(Block, Block),
    Hold(Block, Dangle),
    NotHold(Block, Dangle),
}

// Case analysis

#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub guard: Guard,
    pub block: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    Bool(bool),
    Cmp(CmpOp, OpTyp, Exp),
    Sub(Typ, Box<il::ast::Subcheck>),
    Match(Pattern),
    Mem(Exp),
}

// Instructions

pub type Iid = i64;

pub type Instr = Spanned<Noted<InstrKind, Iid>>;

/// Constructs an instruction
pub fn instr(kind: InstrKind, iid: Iid, span: Span) -> Instr {
    crate::spanned! {
        node: Noted::new(kind, iid),
        span: span,
    }
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
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
    pub iter_exps: Vec<IterExp>,
    pub block: Block,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HoldInstr {
    pub id: Id,
    pub not_exp: NotExp,
    pub iter_exps: Vec<IterExp>,
    pub hold_case: HoldCase,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CaseInstr {
    pub exp: Exp,
    pub cases: Vec<Case>,
    pub dangle: Dangle,
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
    pub iter_instrs: Vec<IterInstr>,
    pub block: Block,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuleInstr {
    pub id: Id,
    pub not_exp: NotExp,
    pub input_hint: InputHint,
    pub iter_instrs: Vec<IterInstr>,
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
pub type ElseBlock = Vec<Instr>;
pub type IterInstr = il::ast::IterPrem;

// Hints

pub type Hint = el::ast::Hint;

// Relations

// not_typ `hint(input` `%`int* `)`
#[derive(Clone, Debug, PartialEq)]
pub struct RelSignature {
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
}

// id `:` rel_signature exp* hint*
#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
    pub hints: Vec<Hint>,
}

// id `:` mixop `hint(input` `%`int* `)` exp* block elseblock? hint*
#[derive(Clone, Debug, PartialEq)]
pub struct Rel {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
    pub block: Block,
    pub else_block: Option<ElseBlock>,
    pub hints: Vec<Hint>,
}

// Functions

// id `<` list(tparam, `,`) `>` list(param, `,`) `:` hint*
#[derive(Clone, Debug, PartialEq)]
pub struct ExternFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

// id `<` list(tparam, `,`) `>` list(param, `,`) `:` hint*
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

// `(` list(exp, `,`)* `)` `->` exp block
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub exps_input: Vec<Exp>,
    pub exp: Exp,
    pub block: Block,
}

// id `(` list(param, `,`) `)` `:` typ tablerow* hint*
#[derive(Clone, Debug, PartialEq)]
pub struct TableFunc {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub table_rows: Vec<TableRow>,
    pub hints: Vec<Hint>,
}

// id `<` list(tparam, `,`) `>` list(arg, `,`) `:` typ block elseblock? hint*
#[derive(Clone, Debug, PartialEq)]
pub struct DefinedFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub block: Block,
    pub else_block: Option<ElseBlock>,
    pub hints: Vec<Hint>,
}

// Definitions

pub type Def = Spanned<DefKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct ExternTypDef {
    pub id: Id,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub def_typ: DefTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarDef {
    pub id: Id,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    // `extern` `syntax` id hint*
    ExternTyp(ExternTypDef),
    // `syntax` id `<` list(tparam, `,`) `>` `=` def_typ hint*
    Typ(TypDef),
    // `var` id `:` typ hint*
    Var(VarDef),
    // `extern` `relation` rel
    ExternRel(ExternRel),
    // `relation` rel
    Rel(Rel),
    // `extern` `dec` externfunc
    ExternDec(ExternFunc),
    // `builtin` `dec` builtinfunc
    BuiltinDec(BuiltinFunc),
    // `tbl` `dec` tablefunc
    TableDec(TableFunc),
    // `dec` func
    FuncDec(DefinedFunc),
}

// Spec

pub type Spec = Vec<Def>;
