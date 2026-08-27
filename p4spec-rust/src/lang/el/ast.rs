//! Elaboration language model

use crate::lang::{
    common::{self, notation::atom, source::Spanned},
    xl::num,
};

// Numbers

pub type Num = num::Number;

// Texts

pub type Text = String;

// Identifiers

pub type Id = common::Id;
pub type IdKind = common::IdKind;

// Atoms

pub type Atom = Spanned<atom::Atom>;

// Iterators

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Iter {
    /// `?`
    Opt,
    /// `*`
    List,
}

// Types

pub type PlainTyp = Spanned<PlainTypKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlainTypKind {
    /// `bool`
    Bool,
    /// `numtyp`
    Num(num::Typ),
    /// `text`
    Text,
    /// `id (`<` list(targ, `,`) `>`)?`
    Var(Id, Vec<Targ>),
    /// `(` plain_typ `)`
    Paren(Box<PlainTyp>),
    /// `(` list(plain_typ, `,`) `)`
    Tuple(Vec<PlainTyp>),
    /// `plain_typ iter`
    Iter(Box<PlainTyp>, Iter),
}

// Operators

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumOp {
    Dec,
    Hex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Bool(crate::lang::xl::bool::UnOp),
    Num(num::UnOp),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Bool(crate::lang::xl::bool::BinOp),
    Num(num::BinOp),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    Bool(crate::lang::xl::bool::CmpOp),
    Num(num::CmpOp),
}

// Expressions

pub type Exp = Spanned<ExpKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpKind {
    /// `bool`
    Bool(bool),
    /// `num`
    Num(NumOp, Num),
    /// `text`
    Text(Text),
    /// `id`
    Var(Id),
    /// `unop exp`
    Un(UnOp, Box<Exp>),
    /// `exp binop exp`
    Bin(Box<Exp>, BinOp, Box<Exp>),
    /// `exp cmpop exp`
    Cmp(Box<Exp>, CmpOp, Box<Exp>),
    /// `$(` exp `)`
    Arith(Box<Exp>),
    /// `eps`
    Eps,
    /// `[` list(exp, `,`) `]`
    List(Vec<Exp>),
    /// `exp :: exp`
    Cons(Box<Exp>, Box<Exp>),
    /// `exp ++ exp`
    Cat(Box<Exp>, Box<Exp>),
    /// `exp [` exp `]`
    Idx(Box<Exp>, Box<Exp>),
    /// `exp [` exp `:` exp `]`
    Slice(Box<Exp>, Box<Exp>, Box<Exp>),
    /// `|` exp `|`
    Len(Box<Exp>),
    /// `exp <- exp`
    Mem(Box<Exp>, Box<Exp>),
    /// `{` list(atom exp, `,`) `}`
    Str(Vec<(Atom, Exp)>),
    /// `exp . atom`
    Dot(Box<Exp>, Atom),
    /// `exp [` path `=` exp `]`
    Upd(Box<Exp>, Path, Box<Exp>),
    /// `(` exp `)`
    Paren(Box<Exp>),
    /// `(` list2(exp, `,`) `)`
    Tuple(Vec<Exp>),
    /// `$` defid (`<` list(targ, `,`) `>`)? (`(` list(arg, `,`) `)`)?
    Call(Id, Vec<Targ>, Vec<Arg>),
    /// `exp iter`
    Iter(Box<Exp>, Iter),
    /// `exp <: typ`
    Sub(Box<Exp>, PlainTyp),
    // Notation expressions
    /// `atom`
    Atom(Atom),
    /// `list(exp, ` `)`
    Seq(Vec<Exp>),
    /// `exp atom exp`
    Infix(Box<Exp>, Atom, Box<Exp>),
    /// ``[({` exp `})]``
    Brack(Atom, Box<Exp>, Atom),
    // Hint expressions
    /// `%N` or `%` or `%%` or `!%`
    Hole(Hole),
    /// `exp # exp`
    Fuse(Box<Exp>, Box<Exp>),
    /// `## exp`
    Unparen(Box<Exp>),
    /// `latex (` `"..."`* `)`
    Latex(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hole {
    Num(i64),
    Next,
    Rest,
    None,
}

// Paths

pub type Path = Spanned<PathKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathKind {
    Root,
    /// `path [` exp `]`
    Idx(Box<Path>, Box<Exp>),
    /// `path [` exp `:` exp `]`
    Slice(Box<Path>, Box<Exp>, Box<Exp>),
    /// `path . atom`
    Dot(Box<Path>, Atom),
}

// Arguments

pub type Arg = Spanned<ArgKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// `exp`
    Exp(Box<Exp>),
    /// `$id`
    Def(Id),
}

// Type arguments

pub type Targ = Spanned<TargKind>;
pub type TargKind = PlainTypKind;

// Hints

pub type Hint = (Id, Exp);

// Notation types

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typ {
    Plain(PlainTyp),
    Notation(NotTyp),
}

pub type NotTyp = Spanned<NotTypKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotTypKind {
    Atom(Atom),
    Seq(Vec<Typ>),
    Infix(Box<Typ>, Atom, Box<Typ>),
    Brack(Atom, Box<Typ>, Atom),
}

pub type DefTyp = Spanned<DefTypKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefTypKind {
    Plain(PlainTyp),
    Struct(Vec<TypField>),
    Variant(Vec<TypCase>),
}

pub type TypField = (Atom, PlainTyp, Vec<Hint>);
pub type TypCase = (Typ, Vec<Hint>);

// Parameters and premises

pub type Param = Spanned<ParamKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamKind {
    Exp(PlainTyp),
    Def(Id, Vec<TParam>, Vec<Param>, PlainTyp),
}

pub type TParam = Spanned<TParamKind>;
pub type TParamKind = IdKind;

pub type Prem = Spanned<PremKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarPrem {
    pub id: Id,
    pub plain_typ: PlainTyp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulePrem {
    pub id: Id,
    pub exp: Exp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleNotPrem {
    pub id: Id,
    pub exp: Exp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfPrem {
    pub exp: Exp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IterPrem {
    pub prem: Box<Prem>,
    pub iter: Iter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugPrem {
    pub exp: Exp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PremKind {
    Var(VarPrem),
    Rule(RulePrem),
    RuleNot(RuleNotPrem),
    If(IfPrem),
    Else,
    Iter(IterPrem),
    Debug(DebugPrem),
}

// Rules and tables

pub type Rule = Spanned<RuleKind>;
pub type RuleKind = (Id, Id, Exp, Vec<Prem>);

pub type TableRow = Spanned<TableRowKind>;
pub type TableRowKind = (Exp, Exp);

// Definitions

pub type Def = Spanned<DefKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternSyntaxDef {
    pub id: Id,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDef {
    pub entries: Vec<SyntaxDefEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxDefEntry {
    pub id: Id,
    pub tparams: Vec<TParam>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub def_typ: DefTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarDef {
    pub id: Id,
    pub plain_typ: PlainTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternRelDef {
    pub id: Id,
    pub not_typ: NotTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelDef {
    pub id: Id,
    pub not_typ: NotTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleGroupDef {
    pub relid: Id,
    pub groupid: Id,
    pub rules: Vec<Rule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternDecDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub plain_typ: PlainTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltinDecDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub plain_typ: PlainTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableDecDef {
    pub id: Id,
    pub params: Vec<Param>,
    pub plain_typ: PlainTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuncDecDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub plain_typ: PlainTyp,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableDef {
    pub id: Id,
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuncDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub args: Vec<Arg>,
    pub exp: Exp,
    pub prems: Vec<Prem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefKind {
    ExternSyntax(ExternSyntaxDef),
    Syntax(SyntaxDef),
    Typ(TypDef),
    Var(VarDef),
    ExternRel(ExternRelDef),
    Rel(RelDef),
    RuleGroup(RuleGroupDef),
    ExternDec(ExternDecDef),
    BuiltinDec(BuiltinDecDef),
    TableDec(TableDecDef),
    FuncDec(FuncDecDef),
    TableDef(TableDef),
    FuncDef(FuncDef),
    Sep,
}

pub type Spec = Vec<Def>;
