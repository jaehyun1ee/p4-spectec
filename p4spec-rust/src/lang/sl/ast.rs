// Keep non-recursive OCaml payloads inline; reserve indirection for
// recursive edges
#![allow(clippy::large_enum_variant)]

use crate::{
    domain::source::{NotePhrase, Phrase},
    lang::{el, hints::input::T as InputHint, il},
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

pub type Vid = il::ast::Vid;
pub type VNote = il::ast::VNote;

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

pub type Param = Phrase<ParamKind>;

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

pub type Case = (Guard, Block);

#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    BoolG(bool),
    CmpG(CmpOp, OpTyp, Exp),
    SubG(Typ),
    MatchG(Pattern),
    MemG(Exp),
}

// Instructions

pub type Iid = i64;

#[derive(Clone, Debug, PartialEq)]
pub struct INote {
    pub iid: Iid,
}

pub type Instr = NotePhrase<InstrKind, INote>;

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
pub type RelSignature = (NotTyp, InputHint);

// id `:` rel_signature exp* hint*
pub type ExternRel = (Id, RelSignature, Vec<Exp>, Vec<Hint>);

// id `:` mixop `hint(input` `%`int* `)` exp* block elseblock? hint*
pub type Rel = (
    Id,
    RelSignature,
    Vec<Exp>,
    Block,
    Option<ElseBlock>,
    Vec<Hint>,
);

// Functions

// id `<` list(tparam, `,`) `>` list(param, `,`) `:` hint*
pub type ExternFunc = (Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>);

// id `<` list(tparam, `,`) `>` list(param, `,`) `:` hint*
pub type BuiltinFunc = (Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>);

// `(` list(exp, `,`)* `)` `->` exp block
pub type TableRow = (Vec<Exp>, Exp, Block);

// id `(` list(param, `,`) `)` `:` typ tablerow* hint*
pub type TableFunc = (Id, Vec<Param>, Typ, Vec<TableRow>, Vec<Hint>);

// id `<` list(tparam, `,`) `>` list(arg, `,`) `:` typ block elseblock? hint*
pub type DefinedFunc = (
    Id,
    Vec<TParam>,
    Vec<Param>,
    Typ,
    Block,
    Option<ElseBlock>,
    Vec<Hint>,
);

// Definitions

pub type Def = Phrase<DefKind>;

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
