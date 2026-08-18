// Keep non-recursive OCaml payloads inline; reserve indirection for
// recursive edges
#![allow(clippy::large_enum_variant)]

use crate::domain::{
    atom::Atom as DomainAtom,
    external_data::ExternalData,
    mixfix::{Mixfix, Mixop as DomainMixop},
    source::{NotePhrase, Phrase},
};
use crate::lang::{el, hints::input::T as InputHint, xl::num};

// Numbers

pub type Num = num::T;

// Texts

pub type Text = String;

// Identifiers

pub type Id = Phrase<IdKind>;
pub type IdKind = String;

// Atoms

pub type Atom = Phrase<AtomKind>;
pub type AtomKind = DomainAtom;

// Mixfix operators

pub type Mixop = DomainMixop;

// Iterators

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Iter {
    /// `?`
    Opt,
    /// `*`
    List,
}

// Variables

pub type Var = (Id, Typ, Vec<Iter>);

// Types

pub type Typ = Phrase<TypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum TypKind {
    /// `bool`
    BoolT,
    /// `numtyp`
    NumT(num::Typ),
    /// `text`
    TextT,
    /// `id (`<` list(targ, `,`) `>`)?`
    VarT(Id, Vec<Targ>),
    /// `(` list(typ, `,`) `)`
    TupleT(Vec<Typ>),
    /// `typ iter`
    IterT(Box<Typ>, Iter),
    /// `<` list(tparam, `,`) `>` `(` list(typ, `,`) `)` `:` typ
    FuncT(Vec<TParam>, Vec<Typ>, Box<Typ>),
}

// Defined types

pub type NotTyp = Phrase<NotTypKind>;
pub type NotTypKind = Mixfix<Typ>;

pub type DefTyp = Phrase<DefTypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefTypKind {
    PlainT(Typ),
    StructT(Vec<TypField>),
    VariantT(Vec<TypCase>),
}

pub type TypField = (Atom, Typ);
pub type TypOrigin = Phrase<TypOriginKind>;
pub type TypOriginKind = (Id, Vec<Targ>);
pub type TypCase = (NotTyp, TypOrigin, Vec<Hint>);

// Values

/// Invariant: `vid` is a globally unique identifier
///
/// Consequently, equal vids imply structural equality. Breaking this invariant
/// would cause silent bugs in value equality by aliasing structurally distinct
/// values.
pub type Vid = i64;

#[derive(Clone, Debug, PartialEq)]
pub struct VNote {
    pub vid: Vid,
    pub typ: TypKind,
    pub vhash: i64,
}

pub type Value = NotePhrase<ValueKind, VNote>;

#[derive(Clone, Debug, PartialEq)]
pub enum ValueKind {
    BoolV(bool),
    NumV(Num),
    TextV(Text),
    StructV(Vec<ValueField>),
    CaseV(Box<ValueCase>),
    TupleV(Vec<Value>),
    OptV(Option<Box<Value>>),
    ListV(Vec<Value>),
    FuncV(Id),
    ExternV(ExternalData),
}

pub type ValueField = (Atom, Value);
pub type ValueCase = Mixfix<Value>;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpTyp {
    BoolT,
    NatT,
    IntT,
}

// Expressions

pub type Exp = NotePhrase<ExpKind, TypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ExpKind {
    /// `bool`
    BoolE(bool),
    /// `num`
    NumE(Num),
    /// `text`
    TextE(Text),
    /// `varid`
    VarE(Id),
    /// `unop exp`
    UnE(UnOp, OpTyp, Box<Exp>),
    /// `exp binop exp`
    BinE(BinOp, OpTyp, Box<Exp>, Box<Exp>),
    /// `exp cmpop exp`
    CmpE(CmpOp, OpTyp, Box<Exp>, Box<Exp>),
    /// `exp as typ`
    UpCastE(Typ, Box<Exp>),
    /// `exp as typ`
    DownCastE(Typ, Box<Exp>),
    /// `exp <: typ`
    SubE(Box<Exp>, Typ),
    /// `exp matches pattern`
    MatchE(Box<Exp>, Pattern),
    /// `(` exp* `)`
    TupleE(Vec<Exp>),
    /// `notexp`
    CaseE(Box<NotExp>),
    /// `{` expfield* `}`
    StrE(Vec<(Atom, Exp)>),
    /// `exp?`
    OptE(Option<Box<Exp>>),
    /// `[` exp* `]`
    ListE(Vec<Exp>),
    /// `exp :: exp`
    ConsE(Box<Exp>, Box<Exp>),
    /// `exp ++ exp`
    CatE(Box<Exp>, Box<Exp>),
    /// `exp <- exp`
    MemE(Box<Exp>, Box<Exp>),
    /// `|` exp `|`
    LenE(Box<Exp>),
    /// `exp.atom`
    DotE(Box<Exp>, Atom),
    /// `exp [` exp `]`
    IdxE(Box<Exp>, Box<Exp>),
    /// `exp [` exp `:` exp `]`
    SliceE(Box<Exp>, Box<Exp>, Box<Exp>),
    /// `exp [` path `=` exp `]`
    UpdE(Box<Exp>, Path, Box<Exp>),
    /// `$id<` targ* `>(` arg* `)`
    CallE(Id, Vec<Targ>, Vec<Arg>),
    /// `exp iterexp`
    IterE(Box<Exp>, IterExp),
}

pub type NotExp = Mixfix<Exp>;
pub type IterExp = (Iter, Vec<Var>);

// Patterns

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    CaseP(Mixop),
    ListP(ListPattern),
    OptP(OptPattern),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListPattern {
    Cons,
    Fixed(i64),
    Nil,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptPattern {
    Some,
    None,
}

// Paths

pub type Path = NotePhrase<PathKind, TypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum PathKind {
    RootP,
    /// `path [` exp `]`
    IdxP(Box<Path>, Box<Exp>),
    /// `path [` exp `:` exp `]`
    SliceP(Box<Path>, Box<Exp>, Box<Exp>),
    /// `path . atom`
    DotP(Box<Path>, Atom),
}

// Parameters

pub type Param = Phrase<ParamKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// `typ`
    ExpP(Typ),
    /// `def $id (<` list(tparam, `,`) `>)? (` list(param, `,`) `)? : typ`
    DefP(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type parameters

pub type TParam = Phrase<TParamKind>;
pub type TParamKind = IdKind;

// Arguments

pub type Arg = Phrase<ArgKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    /// `exp`
    ExpA(Exp),
    /// `$id`
    DefA(Id),
}

// Type arguments

pub type Targ = Phrase<TargKind>;
pub type TargKind = TypKind;

// Premises

pub type Prem = Phrase<PremKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum PremKind {
    /// `id : notexp`
    RulePr(Id, NotExp, InputHint),
    /// `if exp`
    IfPr(Exp),
    /// `if id : notexp holds`
    IfHoldPr(Id, NotExp),
    /// `if id : notexp does not hold`
    IfNotHoldPr(Id, NotExp),
    /// `let exp = exp`
    LetPr(Exp, Exp),
    /// `prem iterprem`
    IterPr(Box<Prem>, IterPrem),
    /// `debug exp`
    DebugPr(Exp),
}

pub type IterPrem = (Iter, Vec<Var>, Vec<Var>);

// Rules

pub type Rule = Phrase<RuleKind>;
pub type RuleKind = (Id, NotExp, Vec<Prem>);

pub type RuleGroup = Phrase<RuleGroupKind>;
pub type RuleGroupKind = (Id, Vec<Rule>);

pub type ElseGroup = Phrase<ElseGroupKind>;
pub type ElseGroupKind = (Id, Rule);

// Clauses

pub type Clause = Phrase<ClauseKind>;
pub type ClauseKind = (Vec<Arg>, Exp, Vec<Prem>);

pub type ElseClause = Clause;
pub type ElseClauseKind = ClauseKind;

// Table rows

pub type TableRow = Phrase<TableRowKind>;
pub type TableRowKind = (Vec<Arg>, Exp);

// Hints

pub type Hint = el::ast::Hint;

// Definitions

pub type Def = Phrase<DefKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    /// `extern syntax id hint*`
    ExternTypD(Id, Vec<Hint>),
    /// `syntax id <` list(tparam, `,`) `> hint* = deftyp`
    TypD(Id, Vec<TParam>, DefTyp, Vec<Hint>),
    /// `var id : typ hint*`
    VarD(Id, Typ, Vec<Hint>),
    /// `extern relation id : nottyp hint(input %int*) hint*`
    ExternRelD(Id, NotTyp, InputHint, Vec<Hint>),
    /// `relation id : nottyp hint(input %int*) rulegroup* hint*`
    RelD(
        Id,
        NotTyp,
        InputHint,
        Vec<RuleGroup>,
        Option<ElseGroup>,
        Vec<Hint>,
    ),
    /// `extern dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    ExternDecD(Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>),
    /// `builtin dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    BuiltinDecD(Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>),
    /// `table dec id list(param, `,`) : typ hint*`
    TableDecD(Id, Vec<Param>, Typ, Vec<TableRow>, Vec<Hint>),
    /// `dec id <` list(tparam, `,`) `> list(param, `,`) : typ clause* hint*`
    FuncDecD(
        Id,
        Vec<TParam>,
        Vec<Param>,
        Typ,
        Vec<Clause>,
        Option<ElseClause>,
        Vec<Hint>,
    ),
}

// Spec

pub type Spec = Vec<Def>;
