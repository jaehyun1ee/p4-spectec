//! Prose language model

use crate::lang::{
    common::{
        notation::mixfix::Mixfix,
        source::{NotePhrase, Phrase},
    },
    pl::annot,
    sl,
};

// Numbers

pub type Num = sl::ast::Num;

// Texts

pub type Text = sl::ast::Text;

// Identifiers

pub type Id = sl::ast::Id;

// Atoms

pub type Atom = sl::ast::Atom;

// Mixfix operators

pub type Mixop = sl::ast::Mixop;

// Iterators

pub type Iter = sl::ast::Iter;

// Variables

pub type Var = sl::ast::Var;

// Types

pub type Typ = sl::ast::Typ;
pub type TypKind = sl::ast::TypKind;
pub type NotTyp = sl::ast::NotTyp;
pub type NotTypKind = sl::ast::NotTypKind;
pub type DefTyp = sl::ast::DefTyp;
pub type DefTypKind = sl::ast::DefTypKind;
pub type TypField = sl::ast::TypField;
pub type TypCase = sl::ast::TypCase;

// Values

pub type Value = sl::ast::Value;

// Operators

pub type UnOp = sl::ast::UnOp;
pub type BinOp = sl::ast::BinOp;
pub type CmpOp = sl::ast::CmpOp;
pub type OpTyp = sl::ast::OpTyp;

// Subtype checks

pub type Subcheck = sl::ast::Subcheck;

// Expressions

pub type ExpNode = NotePhrase<ExpKind, TypKind>;
pub type Exp = annot::Annotated<ExpNode>;
#[derive(Clone, Debug, PartialEq)]
pub enum ExpKind {
    Bool(bool),
    Num(Num),
    Text(Text),
    Var(Id),
    Un(UnOp, OpTyp, Box<Exp>),
    Bin(BinOp, OpTyp, Box<Exp>, Box<Exp>),
    Cmp(CmpOp, OpTyp, Box<Exp>, Box<Exp>),
    UpCast(Typ, Box<Exp>),
    DownCast(Typ, Box<Exp>),
    Sub(Box<Exp>, Typ, Box<Subcheck>),
    Match(Box<Exp>, Pattern),
    Tuple(Vec<Exp>),
    Case(Box<NotExp>),
    Str(Vec<(Atom, Exp)>),
    Opt(Option<Box<Exp>>),
    List(Vec<Exp>),
    Cons(Box<Exp>, Box<Exp>),
    Cat(Box<Exp>, Box<Exp>),
    Mem(Box<Exp>, Box<Exp>),
    Len(Box<Exp>),
    Dot(Box<Exp>, Atom),
    Idx(Box<Exp>, Box<Exp>),
    Slice(Box<Exp>, Box<Exp>, Box<Exp>),
    Upd(Box<Exp>, Box<Path>, Box<Exp>),
    Call(Id, Vec<Targ>, Vec<Arg>),
    Iter(Box<Exp>, ExpIter),
}
pub type NotExp = Mixfix<Exp>;
pub type ExpIter = sl::ast::ExpIter;

// Patterns

pub type Pattern = sl::ast::Pattern;

// Path

pub type Path = NotePhrase<PathKind, TypKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum PathKind {
    Root,
    Idx(Box<Path>, Box<Exp>),
    Slice(Box<Path>, Box<Exp>, Box<Exp>),
    Dot(Box<Path>, Atom),
}

// Type parameters

pub type TParam = sl::ast::TParam;

// Parameters

pub type Param = Phrase<ParamKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    Exp(Typ, Box<Exp>),
    Def(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type arguments

pub type Targ = sl::ast::Targ;

// Arguments

pub type Arg = Phrase<ArgKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    Exp(Box<Exp>),
    Def(Id),
}

// Dangling

pub type Dangle = sl::ast::Dangle;

// Holding conditions

#[derive(Clone, Debug, PartialEq)]
pub enum HoldCase<Tier> {
    Both(Block<Tier>, Block<Tier>),
    Hold(Block<Tier>, Dangle),
    NotHold(Block<Tier>, Dangle),
}

// Case analysis

#[derive(Clone, Debug, PartialEq)]
pub struct Case<Tier> {
    pub guard: Guard,
    pub block: Block<Tier>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    Bool(bool),
    Cmp(CmpOp, OpTyp, Exp),
    Sub(Typ, Box<Subcheck>),
    Match(Pattern),
    Mem(Exp),
    // Shorthands
    CheckLetSub(Typ, Box<Subcheck>, Exp),
    CheckLetMatch(Pattern, Exp),
}

// Instructions

#[derive(Clone, Debug, PartialEq)]
pub enum Fallthrough {
    FallGroup(Id),
    FallNext,
    FallElse,
    FallFail,
}

pub type InstrNode<Tier> = NotePhrase<InstrKind<Tier>, Option<Fallthrough>>;
pub type Instr<Tier> = annot::Annotated<InstrNode<Tier>>;

#[derive(Clone, Debug, PartialEq)]
pub enum InstrKind<Tier> {
    If(IfInstr<Tier>),
    Hold(HoldInstr<Tier>),
    Case(CaseInstr<Tier>),
    Let(LetInstr),
    Debug(DebugInstr),
    Destruct(DestructInstr),
    CheckLetSub(CheckLetSubInstr<Tier>),
    CheckLetMatch(CheckLetMatchInstr<Tier>),
    OptionGet(OptionGetInstr<Tier>),
    Tier(TierInstr<Tier>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfInstr<Tier> {
    pub exp: Exp,
    pub iter_exps: Vec<ExpIter>,
    pub block: Block<Tier>,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct HoldInstr<Tier> {
    pub id: Id,
    pub not_exp: NotExp,
    pub iter_exps: Vec<ExpIter>,
    pub hold_case: HoldCase<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CaseInstr<Tier> {
    pub exp: Exp,
    pub cases: Vec<Case<Tier>>,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LetInstr {
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub iter_instrs: Vec<InstrIter>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugInstr {
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DestructInstr {
    pub bindings: Vec<(Option<String>, Exp)>,
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CheckLetSubInstr<Tier> {
    pub typ: Typ,
    pub subcheck: Box<Subcheck>,
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub block: Block<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CheckLetMatchInstr<Tier> {
    pub pattern: Pattern,
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub block: Block<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct OptionGetInstr<Tier> {
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub block: Block<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TierInstr<Tier> {
    pub tier: Tier,
}

pub type Block<Tier> = Vec<Instr<Tier>>;
pub type InstrIter = sl::ast::InstrIter;

// Relations

pub type RelSignature = sl::ast::RelSignature;

// Group-body tier

#[derive(Clone, Debug, PartialEq)]
pub enum InstrGroup {
    Result(ResultGroupInstr),
    Return(ReturnGroupInstr),
    Rule(RuleGroupInstr),
    Backtrack(BacktrackGroupInstr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultGroupInstr {
    pub rel_signature: RelSignature,
    pub exps_output: Vec<Exp>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ReturnGroupInstr {
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuleGroupInstr {
    pub id: Id,
    pub not_exp: NotExp,
    pub input_hint: crate::lang::hints::input::InputHint,
    pub iter_instrs: Vec<InstrIter>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BacktrackGroupInstr {
    pub blocks: Vec<BlockGroup>,
}

pub type BlockGroup = Block<InstrGroup>;

// Dispatch tier

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum InstrDispatch {
    Group(GroupDispatchInstr),
    Route(RouteDispatchInstr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupDispatchInstr {
    pub id_rel: Id,
    pub id_group: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
    pub block: BlockGroup,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RouteDispatchInstr {
    pub blocks: Vec<BlockDispatch>,
}

pub type BlockDispatch = Block<InstrDispatch>;

// Relations

#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rel {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
    pub block: BlockDispatch,
    pub block_else_opt: Option<BlockDispatch>,
}

// Functions

#[derive(Clone, Debug, PartialEq)]
pub struct ExternFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub exps_input: Vec<Exp>,
    pub exp: Exp,
    pub block: BlockGroup,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableFunc {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub block: BlockGroup,
    pub block_else_opt: Option<BlockGroup>,
}

// Definitions

pub type DefNode = Phrase<DefKind>;
pub type Def = annot::Annotated<DefNode>;

#[derive(Clone, Debug, PartialEq)]
pub struct ExternTypDef {
    pub id: Id,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub def_typ: DefTyp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VarDef {
    pub id: Id,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    ExternTyp(ExternTypDef),
    Typ(TypDef),
    Var(VarDef),
    ExternRel(ExternRel),
    Rel(Rel),
    ExternDec(ExternFunc),
    BuiltinDec(BuiltinFunc),
    TableDec(TableFunc),
    FuncDec(DefinedFunc),
}

// Spec

pub type Spec = Vec<Def>;
