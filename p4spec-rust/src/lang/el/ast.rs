// Keep non-recursive OCaml payloads inline; reserve indirection for
// recursive edges
#![allow(clippy::large_enum_variant)]

use crate::domain::{atom::Atom as DomainAtom, source::Spanned};
use crate::lang::xl::num;

// Numbers

pub type Num = num::T;

// Texts

pub type Text = String;

// Identifiers

pub type Id = Spanned<IdKind>;
pub type IdKind = String;

// Atoms

pub type Atom = Spanned<DomainAtom>;

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
    BoolT,
    /// `numtyp`
    NumT(num::Typ),
    /// `text`
    TextT,
    /// `id (`<` list(targ, `,`) `>`)?`
    VarT(Id, Vec<Targ>),
    /// `(` plaintyp `)`
    ParenT(Box<PlainTyp>),
    /// `(` list(plaintyp, `,`) `)`
    TupleT(Vec<PlainTyp>),
    /// `plaintyp iter`
    IterT(Box<PlainTyp>, Iter),
}

// Operators

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumOp {
    DecOp,
    HexOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    NotOp,
    PlusOp,
    MinusOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    AndOp,
    OrOp,
    ImplOp,
    EquivOp,
    AddOp,
    SubOp,
    MulOp,
    DivOp,
    ModOp,
    PowOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    EqOp,
    NeOp,
    LtOp,
    GtOp,
    LeOp,
    GeOp,
}

// Expressions

pub type Exp = Spanned<ExpKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpKind {
    /// `bool`
    BoolE(bool),
    /// `num`
    NumE(NumOp, Num),
    /// `text`
    TextE(Text),
    /// `id`
    VarE(Id),
    /// `unop exp`
    UnE(UnOp, Box<Exp>),
    /// `exp binop exp`
    BinE(Box<Exp>, BinOp, Box<Exp>),
    /// `exp cmpop exp`
    CmpE(Box<Exp>, CmpOp, Box<Exp>),
    /// `$(` exp `)`
    ArithE(Box<Exp>),
    /// `eps`
    EpsE,
    /// `[` list(exp, `,`) `]`
    ListE(Vec<Exp>),
    /// `exp :: exp`
    ConsE(Box<Exp>, Box<Exp>),
    /// `exp ++ exp`
    CatE(Box<Exp>, Box<Exp>),
    /// `exp [` exp `]`
    IdxE(Box<Exp>, Box<Exp>),
    /// `exp [` exp `:` exp `]`
    SliceE(Box<Exp>, Box<Exp>, Box<Exp>),
    /// `|` exp `|`
    LenE(Box<Exp>),
    /// `exp <- exp`
    MemE(Box<Exp>, Box<Exp>),
    /// `{` list(atom exp, `,`) `}`
    StrE(Vec<(Atom, Exp)>),
    /// `exp . atom`
    DotE(Box<Exp>, Atom),
    /// `exp [` path `=` exp `]`
    UpdE(Box<Exp>, Path, Box<Exp>),
    /// `(` exp `)`
    ParenE(Box<Exp>),
    /// `(` list2(exp, `,`) `)`
    TupleE(Vec<Exp>),
    /// `$` defid (`<` list(targ, `,`) `>`)? (`(` list(arg, `,`) `)`)?
    CallE(Id, Vec<Targ>, Vec<Arg>),
    /// `exp iter`
    IterE(Box<Exp>, Iter),
    /// `exp <: typ`
    SubE(Box<Exp>, PlainTyp),

    // Notation expressions
    /// `atom`
    AtomE(Atom),
    /// `list(exp, ` `)`
    SeqE(Vec<Exp>),
    /// `exp atom exp`
    InfixE(Box<Exp>, Atom, Box<Exp>),
    /// ``[({` exp `})]``
    BrackE(Atom, Box<Exp>, Atom),

    // Hint expressions
    /// `%N` or `%` or `%%` or `!%`
    HoleE(Hole),
    /// `exp # exp`
    FuseE(Box<Exp>, Box<Exp>),
    /// `## exp`
    UnparenE(Box<Exp>),
    /// `latex (` `"..."`* `)`
    LatexE(String),
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
    RootP,
    /// `path [` exp `]`
    IdxP(Box<Path>, Box<Exp>),
    /// `path [` exp `:` exp `]`
    SliceP(Box<Path>, Box<Exp>, Box<Exp>),
    /// `path . atom`
    DotP(Box<Path>, Atom),
}

// Arguments

pub type Arg = Spanned<ArgKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgKind {
    /// `exp`
    ExpA(Exp),
    /// `$id`
    DefA(Id),
}

// Type arguments

pub type Targ = Spanned<TargKind>;
pub type TargKind = PlainTypKind;

// Hints

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub hintid: Id,
    pub hintexp: Exp,
}

// Notation types

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typ {
    PlainT(PlainTyp),
    NotationT(NotTyp),
}

pub type NotTyp = Spanned<NotTypKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotTypKind {
    AtomT(Atom),
    SeqT(Vec<Typ>),
    InfixT(Box<Typ>, Atom, Box<Typ>),
    BrackT(Atom, Box<Typ>, Atom),
}

pub type DefTyp = Spanned<DefTypKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefTypKind {
    PlainTD(PlainTyp),
    StructTD(Vec<TypField>),
    VariantTD(Vec<TypCase>),
}

pub type TypField = (Atom, PlainTyp, Vec<Hint>);
pub type TypCase = (Typ, Vec<Hint>);

// Parameters and premises

pub type Param = Spanned<ParamKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamKind {
    ExpP(PlainTyp),
    DefP(Id, Vec<TParam>, Vec<Param>, PlainTyp),
}

pub type TParam = Spanned<TParamKind>;
pub type TParamKind = IdKind;

pub type Prem = Spanned<PremKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PremKind {
    VarPr(Id, PlainTyp),
    RulePr(Id, Exp),
    RuleNotPr(Id, Exp),
    IfPr(Exp),
    ElsePr,
    IterPr(Box<Prem>, Iter),
    DebugPr(Exp),
}

// Rules and tables

pub type Rule = Spanned<RuleKind>;
pub type RuleKind = (Id, Id, Exp, Vec<Prem>);

pub type TableRow = Spanned<TableRowKind>;
pub type TableRowKind = (Exp, Exp);

// Definitions

pub type Def = Spanned<DefKind>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefKind {
    ExternSynD(Id, Vec<Hint>),
    SynD(Vec<(Id, Vec<TParam>)>),
    TypD(Id, Vec<TParam>, DefTyp, Vec<Hint>),
    VarD(Id, PlainTyp, Vec<Hint>),
    ExternRelD(Id, NotTyp, Vec<Hint>),
    RelD(Id, NotTyp, Vec<Hint>),
    RuleGroupD(Id, Id, Vec<Rule>),
    ExternDecD(Id, Vec<TParam>, Vec<Param>, PlainTyp, Vec<Hint>),
    BuiltinDecD(Id, Vec<TParam>, Vec<Param>, PlainTyp, Vec<Hint>),
    TableDecD(Id, Vec<Param>, PlainTyp, Vec<Hint>),
    FuncDecD(Id, Vec<TParam>, Vec<Param>, PlainTyp, Vec<Hint>),
    TableDefD(Id, Vec<TableRow>),
    FuncDefD(Id, Vec<TParam>, Vec<Arg>, Exp, Vec<Prem>),
    SepD,
}

pub type Spec = Vec<Def>;
