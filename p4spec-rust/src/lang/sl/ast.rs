//! Structured language model

use crate::lang::{common::source::Phrase, el, hints::input::InputHint, il};

// Numbers

pub type Num = il::ast::Num;

// Texts

pub type Text = il::ast::Text;

// Identifiers

pub type Id = il::ast::Id;

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

pub type Exp<I = Id, V = Var> = il::ast::Exp<I, V>;
pub type ExpKind<I = Id, V = Var> = il::ast::ExpKind<I, V>;

pub type NotExp<I = Id, V = Var> = il::ast::NotExp<I, V>;
pub type ExpIter<V = Var> = il::ast::ExpIter<V>;

// Patterns

pub type Pattern = il::ast::Pattern;

// Path

pub type Path<I = Id, V = Var> = il::ast::Path<I, V>;
pub type PathKind<I = Id, V = Var> = il::ast::PathKind<I, V>;

// Type parameters

pub type TParam = il::ast::TParam;

// Parameters

pub type Param<I = Id, V = Var> = Phrase<ParamKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind<I = Id, V = Var> {
    Exp(Typ, Box<Exp<I, V>>),
    Def(Id, Vec<TParam>, Vec<Param<I, V>>, Typ),
}

// Type arguments

pub type Targ = il::ast::Targ;
pub type TargKind = il::ast::TargKind;

// Arguments

pub type Arg<I = Id, V = Var> = il::ast::Arg<I, V>;
pub type ArgKind<I = Id, V = Var> = il::ast::ArgKind<I, V>;

// Dangling

pub type Dangle = bool;

// Holding conditions

#[derive(Clone, Debug, PartialEq)]
pub enum HoldCase<I = Id, V = Var> {
    Both(Block<I, V>, Block<I, V>),
    Hold(Block<I, V>, Dangle),
    NotHold(Block<I, V>, Dangle),
}

// Case analysis

#[derive(Clone, Debug, PartialEq)]
pub struct Case<I = Id, V = Var> {
    pub guard: Guard<I, V>,
    pub block: Block<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Guard<I = Id, V = Var> {
    Bool(bool),
    Cmp(CmpOp, OpTyp, Exp<I, V>),
    Sub(Typ, Box<il::ast::Subcheck>),
    Match(Pattern),
    Mem(Exp<I, V>),
}

// Instructions

pub type Instr<I = Id, V = Var> = Phrase<InstrKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum InstrKind<I = Id, V = Var> {
    If(IfInstr<I, V>),
    Hold(HoldInstr<I, V>),
    Case(CaseInstr<I, V>),
    Group(GroupInstr<I, V>),
    Let(LetInstr<I, V>),
    Rule(RuleInstr<I, V>),
    Result(ResultInstr<I, V>),
    Return(ReturnInstr<I, V>),
    Debug(DebugInstr<I, V>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfInstr<I = Id, V = Var> {
    pub exp: Exp<I, V>,
    pub iter_exps: Vec<ExpIter<V>>,
    pub block: Block<I, V>,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HoldInstr<I = Id, V = Var> {
    pub id: Id,
    pub not_exp: NotExp<I, V>,
    pub iter_exps: Vec<ExpIter<V>>,
    pub hold_case: HoldCase<I, V>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CaseInstr<I = Id, V = Var> {
    pub exp: Exp<I, V>,
    pub cases: Vec<Case<I, V>>,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct GroupInstr<I = Id, V = Var> {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps: Vec<Exp<I, V>>,
    pub block: Block<I, V>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LetInstr<I = Id, V = Var> {
    pub exp_l: Exp<I, V>,
    pub exp_r: Exp<I, V>,
    pub iter_instrs: Vec<InstrIter<V>>,
    pub block: Block<I, V>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuleInstr<I = Id, V = Var> {
    pub id: Id,
    pub not_exp: NotExp<I, V>,
    pub input_hint: InputHint,
    pub iter_instrs: Vec<InstrIter<V>>,
    pub block: Block<I, V>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResultInstr<I = Id, V = Var> {
    pub rel_signature: RelSignature,
    pub exps: Vec<Exp<I, V>>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnInstr<I = Id, V = Var> {
    pub exp: Exp<I, V>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugInstr<I = Id, V = Var> {
    pub exp: Exp<I, V>,
    pub instr: Box<Instr<I, V>>,
}

pub type Block<I = Id, V = Var> = Vec<Instr<I, V>>;
pub type ElseBlock<I = Id, V = Var> = Vec<Instr<I, V>>;
pub type InstrIter<V = Var> = il::ast::PremIter<V>;

// Hints

pub type Hint = el::ast::Hint;

// Type definitions

#[derive(Clone, Debug, PartialEq)]
pub enum TypDef {
    /// `extern syntax id hint*`
    Extern(ExternTyp),
    /// `syntax id <` list(tparam, `,`) `> : typ hint*`
    Defined(Box<DefinedTyp>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternTyp {
    pub id: Id,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedTyp {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub def_typ: DefTyp,
    pub hints: Vec<Hint>,
}

// Meta-variables

#[derive(Clone, Debug, PartialEq)]
pub struct VarDef {
    pub id: Id,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

// Relations

#[derive(Clone, Debug, PartialEq)]
pub enum RelDef<I = Id, V = Var> {
    /// `extern relation id : not_typ hint(input %int*) hint*`
    Extern(ExternRel<I, V>),
    /// `relation id : not_typ hint(input %int*) rulegroup* hint*`
    Defined(DefinedRel<I, V>),
}

// not_typ `hint(input` `%`int* `)`
#[derive(Clone, Debug, PartialEq)]
pub struct RelSignature {
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
}

// id `:` rel_signature exp* hint*
#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel<I = Id, V = Var> {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp<I, V>>,
    pub hints: Vec<Hint>,
}

// id `:` mixop `hint(input` `%`int* `)` exp* block elseblock? hint*
#[derive(Clone, Debug, PartialEq)]
pub struct DefinedRel<I = Id, V = Var> {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp<I, V>>,
    pub block: Block<I, V>,
    pub block_else: Option<ElseBlock<I, V>>,
    pub hints: Vec<Hint>,
}

// Meta-functions

#[derive(Clone, Debug, PartialEq)]
pub enum MetaFuncDef<I = Id, V = Var> {
    /// `extern dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Extern(ExternFunc<I, V>),
    /// `builtin dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Builtin(BuiltinFunc<I, V>),
    /// `table dec id list(param, `,`) : typ hint*`
    Table(TableFunc<I, V>),
    /// `dec id <` list(tparam, `,`) `> list(param, `,`) : typ clause* hint*`
    Defined(DefinedFunc<I, V>),
}

// id `<` list(tparam, `,`) `>` list(param, `,`) `:` hint*
#[derive(Clone, Debug, PartialEq)]
pub struct ExternFunc<I = Id, V = Var> {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param<I, V>>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

// id `<` list(tparam, `,`) `>` list(param, `,`) `:` hint*
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinFunc<I = Id, V = Var> {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param<I, V>>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

// `(` list(exp, `,`)* `)` `->` exp block
#[derive(Clone, Debug, PartialEq)]
pub struct TableRow<I = Id, V = Var> {
    pub exps_input: Vec<Exp<I, V>>,
    pub exp: Exp<I, V>,
    pub block: Block<I, V>,
}

// id `(` list(param, `,`) `)` `:` typ tablerow* hint*
#[derive(Clone, Debug, PartialEq)]
pub struct TableFunc<I = Id, V = Var> {
    pub id: Id,
    pub params: Vec<Param<I, V>>,
    pub typ: Typ,
    pub table_rows: Vec<TableRow<I, V>>,
    pub hints: Vec<Hint>,
}

// id `<` list(tparam, `,`) `>` list(arg, `,`) `:` typ block elseblock? hint*
#[derive(Clone, Debug, PartialEq)]
pub struct DefinedFunc<I = Id, V = Var> {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param<I, V>>,
    pub typ: Typ,
    pub block: Block<I, V>,
    pub block_else: Option<ElseBlock<I, V>>,
    pub hints: Vec<Hint>,
}

// Definitions

pub type Def<I = Id, V = Var> = Phrase<DefKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind<I = Id, V = Var> {
    Typ(TypDef),
    // `var` id `:` typ hint*
    Var(VarDef),
    Rel(RelDef<I, V>),
    MetaFunc(MetaFuncDef<I, V>),
}

// Spec

pub type Spec<I = Id, V = Var> = Vec<Def<I, V>>;
