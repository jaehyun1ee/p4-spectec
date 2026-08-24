// Keep non-recursive OCaml payloads inline; reserve indirection for
// recursive edges
#![allow(clippy::large_enum_variant)]

use crate::{
    domain::source::{HasSpan, Span, Spanned},
    lang::{el, hints::input::InputHint, il},
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
pub type AtomKind = il::ast::AtomKind;

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
    ExpP(Typ, Exp),
    DefP(Id, Vec<TParam>, Vec<Param>, Typ),
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
    BothH(Block, Block),
    HoldH(Block, Dangle),
    NotHoldH(Block, Dangle),
}

// Case analysis

#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub guard: Guard,
    pub block: Block,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    BoolG(bool),
    CmpG(CmpOp, OpTyp, Exp),
    SubG(Typ, Box<il::ast::Subcheck>),
    MatchG(Pattern),
    MemG(Exp),
}

// Instructions

pub type Iid = i64;

#[derive(Clone, Debug, PartialEq)]
pub struct Instr {
    pub kind: InstrKind,
    pub iid: Iid,
    pub span: Span,
}

impl Instr {
    pub fn new(kind: InstrKind, iid: Iid, span: Span) -> Self {
        Self { kind, iid, span }
    }
}

impl HasSpan for Instr {
    fn span(&self) -> &Span {
        &self.span
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstrKind {
    // Branching instructions
    IfI(Exp, Vec<IterExp>, Block, Dangle),
    HoldI(Id, NotExp, Vec<IterExp>, HoldCase),
    CaseI(Exp, Vec<Case>, Dangle),
    // Aggregate instructions
    GroupI(Id, RelSignature, Vec<Exp>, Block),
    // Binding instructions
    LetI(Exp, Exp, Vec<IterInstr>, Block),
    RuleI(Id, NotExp, InputHint, Vec<IterInstr>, Block),
    // Result/Return instructions
    ResultI(RelSignature, Vec<Exp>),
    ReturnI(Exp),
    // Debugging instructions
    DebugI(Exp, Box<Instr>),
}

pub type Block = Vec<Instr>;
pub type ElseBlock = Vec<Instr>;
pub type IterInstr = il::ast::IterPrem;

// Hints

pub type Hint = el::ast::Hint;

// Relations

// nottyp `hint(input` `%`int* `)`
#[derive(Clone, Debug, PartialEq)]
pub struct RelSignature {
    pub notation: NotTyp,
    pub input_hint: InputHint,
}

// id `:` rel_signature exp* hint*
#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel {
    pub id: Id,
    pub signature: RelSignature,
    pub inputs: Vec<Exp>,
    pub hints: Vec<Hint>,
}

// id `:` mixop `hint(input` `%`int* `)` exp* block elseblock? hint*
#[derive(Clone, Debug, PartialEq)]
pub struct Rel {
    pub id: Id,
    pub signature: RelSignature,
    pub inputs: Vec<Exp>,
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
    pub inputs: Vec<Exp>,
    pub expression: Exp,
    pub block: Block,
}

// id `(` list(param, `,`) `)` `:` typ tablerow* hint*
#[derive(Clone, Debug, PartialEq)]
pub struct TableFunc {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub rows: Vec<TableRow>,
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
pub enum DefKind {
    // `extern` `syntax` id hint*
    ExternTypD(Id, Vec<Hint>),
    // `syntax` id `<` list(tparam, `,`) `>` `=` deftyp hint*
    TypD(Id, Vec<TParam>, DefTyp, Vec<Hint>),
    // `var` id `:` typ hint*
    VarD(Id, Typ, Vec<Hint>),
    // `extern` `relation` rel
    ExternRelD(ExternRel),
    // `relation` rel
    RelD(Rel),
    // `extern` `dec` externfunc
    ExternDecD(ExternFunc),
    // `builtin` `dec` builtinfunc
    BuiltinDecD(BuiltinFunc),
    // `tbl` `dec` tablefunc
    TableDecD(TableFunc),
    // `dec` func
    FuncDecD(DefinedFunc),
}

// Spec

pub type Spec = Vec<Def>;
