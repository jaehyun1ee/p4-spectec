//! Intermediate-language syntax and executable values.
//!
//! Definitions and expressions carry their stage-specific annotations, while
//! runtime values use immutable shared nodes so recursive children and cache
//! keys clone in constant time. For example, cloning a list value shares its
//! element tree while retaining the list's runtime type and source span.

use std::rc::Rc;

use crate::lang::{
    common::{
        self,
        notation::{atom, mixfix::Mixfix, mixop},
        source::{NotePhrase, Phrase},
    },
    data, el,
    hints::input::InputHint,
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

pub type Atom = Phrase<atom::Atom>;

// Mixfix operators

pub type Mixop = mixop::Mixop;

// Iterators

pub type Iter = common::Iter;

// Variables

#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    pub id: Id,
    pub typ: Typ,
    pub iters: Vec<Iter>,
}

// Types

pub type Typ = data::typ::Typ;
pub type TypKind = data::typ::TypKind;
pub type FuncTyp = data::typ::FuncTyp;

// Subtype checks

#[derive(Clone, Debug, PartialEq)]
pub enum Subcheck {
    Skip,
    Mixop(Vec<Mixop>),
    Tuple(Vec<Subcheck>),
    Iter(Iter, Box<Subcheck>),
    Recurse(Typ),
}

// Defined types

pub type NotTyp = Phrase<NotTypKind>;
pub type NotTypKind = Mixfix<Typ>;

pub type DefTyp = Phrase<DefTypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefTypKind {
    Plain(Typ),
    Struct(Vec<TypField>),
    Variant(Vec<TypCase>),
}

pub type TypField = (Phrase<atom::Atom>, Typ);
pub type TypOrigin = Phrase<TypOriginKind>;
pub type TypOriginKind = (Id, Vec<Targ>);
pub type TypCase = (NotTyp, TypOrigin, Vec<Hint>);

// == Values

pub type Value = data::value::Value;
pub type ValueKind = data::value::ValueKind;
pub type ValueField = data::value::ValueField;
pub type ValueCase = data::value::ValueCase;

// Operators

pub type NumOp = el::ast::NumOp;
pub type UnOp = el::ast::UnOp;
pub type BinOp = el::ast::BinOp;
pub type CmpOp = el::ast::CmpOp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpTyp {
    Bool,
    Nat,
    Int,
}

// Expressions

pub type Exp = NotePhrase<ExpKind, Rc<TypKind>>;

#[derive(Clone, Debug, PartialEq)]
pub enum ExpKind {
    /// `bool`
    Bool(bool),
    /// `num`
    Num(Num),
    /// `text`
    Text(Text),
    /// `varid`
    Var(Id),
    /// `unop exp`
    Un(UnOp, OpTyp, Box<Exp>),
    /// `exp binop exp`
    Bin(BinOp, OpTyp, Box<Exp>, Box<Exp>),
    /// `exp cmpop exp`
    Cmp(CmpOp, OpTyp, Box<Exp>, Box<Exp>),
    /// `exp as typ`
    UpCast(Box<Typ>, Box<Exp>),
    /// `exp as typ`
    DownCast(Box<Typ>, Box<Exp>),
    /// `exp <: typ`
    Sub(Box<Exp>, Box<Typ>, Box<Subcheck>),
    /// `exp matches pattern`
    Match(Box<Exp>, Pattern),
    /// `(` exp* `)`
    Tuple(Vec<Exp>),
    /// `notexp`
    Case(Box<NotExp>),
    /// `{` expfield* `}`
    Str(Vec<ExpField>),
    /// `exp?`
    Opt(Option<Box<Exp>>),
    /// `[` exp* `]`
    List(Vec<Exp>),
    /// `exp :: exp`
    Cons(Box<Exp>, Box<Exp>),
    /// `exp ++ exp`
    Cat(Box<Exp>, Box<Exp>),
    /// `exp <- exp`
    Mem(Box<Exp>, Box<Exp>),
    /// `|` exp `|`
    Len(Box<Exp>),
    /// `exp.atom`
    Dot(Box<Exp>, Atom),
    /// `exp [` exp `]`
    Idx(Box<Exp>, Box<Exp>),
    /// `exp [` exp `:` exp `]`
    Slice(Box<Exp>, Box<Exp>, Box<Exp>),
    /// `exp [` path `=` exp `]`
    Upd(Box<Exp>, Box<Path>, Box<Exp>),
    /// `$id<` targ* `>(` arg* `)`
    Call(Id, Vec<Targ>, Vec<Arg>),
    /// `exp iterexp`
    Iter(Box<Exp>, ExpIter),
}

pub type NotExp = Mixfix<Exp>;
pub type ExpField = (Atom, Exp);
pub type ExpIter = (Iter, Vec<Var>);

// Patterns

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Case(Box<Mixop>),
    List(ListPattern),
    Opt(OptPattern),
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

pub type Path = NotePhrase<PathKind, Rc<TypKind>>;

#[derive(Clone, Debug, PartialEq)]
pub enum PathKind {
    Root,
    /// `path [` exp `]`
    Idx(Box<Path>, Box<Exp>),
    /// `path [` exp `:` exp `]`
    Slice(Box<Path>, Box<Exp>, Box<Exp>),
    /// `path . atom`
    Dot(Box<Path>, Atom),
}

// Parameters

pub type Param = Phrase<ParamKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// `typ`
    Exp(Typ),
    /// `def $id (<` list(tparam, `,`) `>)? (` list(param, `,`) `)? : typ`
    Def(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type parameters

pub type TParam = common::TId;
pub type TParamKind = IdKind;

// Arguments

pub type Arg = Phrase<ArgKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    /// `exp`
    Exp(Box<Exp>),
    /// `$id`
    Def(Id),
}

// Type arguments

pub type Targ = Typ;
pub type TargKind = TypKind;

// Premises

pub type Prem = Phrase<PremKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct RulePrem {
    pub id: Id,
    pub not_exp: NotExp,
    pub input_hint: InputHint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfPrem {
    pub exp: Exp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfHoldPrem {
    pub id: Id,
    pub not_exp: NotExp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfNotHoldPrem {
    pub id: Id,
    pub not_exp: NotExp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IterPrem {
    pub prem: Box<Prem>,
    pub prem_iter: PremIter,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugPrem {
    pub exp: Exp,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum PremKind {
    /// `id : notexp`
    Rule(RulePrem),
    /// `if exp`
    If(IfPrem),
    /// `if id : notexp holds`
    IfHold(IfHoldPrem),
    /// `if id : notexp does not hold`
    IfNotHold(IfNotHoldPrem),
    /// `prem iterprem`
    Iter(IterPrem),
    /// `debug exp`
    Debug(DebugPrem),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PremIter {
    pub iter: Iter,
    pub vars_bound: Vec<Var>,
    pub vars_bind: Vec<Var>,
}

// Rules

pub type Rule = Phrase<RuleKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct RuleKind {
    pub id: Id,
    pub not_exp: NotExp,
    pub prems: Vec<Prem>,
}

pub type RuleGroup = Phrase<RuleGroupKind>;
pub type RuleGroupKind = (Id, Vec<Rule>);

pub type ElseGroup = Phrase<ElseGroupKind>;
pub type ElseGroupKind = (Id, Rule);

// Clauses

pub type Clause = Phrase<ClauseKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct ClauseKind {
    pub args: Vec<Arg>,
    pub expression: Exp,
    pub premises: Vec<Prem>,
}

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
pub struct ExternTyp {
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
pub struct ExternRel {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rel {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub rule_groups: Vec<RuleGroup>,
    pub else_group: Option<ElseGroup>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternDec {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinDec {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableDec {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub rows: Vec<TableRow>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDec {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub clauses: Vec<Clause>,
    pub else_clause: Option<ElseClause>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum DefKind {
    /// `extern syntax id hint*`
    ExternTyp(ExternTyp),
    /// `syntax id <` list(tparam, `,`) `> hint* = def_typ`
    Typ(TypDef),
    /// `var id : typ hint*`
    Var(VarDef),
    /// `extern relation id : not_typ hint(input %int*) hint*`
    ExternRel(ExternRel),
    /// `relation id : not_typ hint(input %int*) rulegroup* hint*`
    Rel(Rel),
    /// `extern dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    ExternDec(ExternDec),
    /// `builtin dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    BuiltinDec(BuiltinDec),
    /// `table dec id list(param, `,`) : typ hint*`
    TableDec(TableDec),
    /// `dec id <` list(tparam, `,`) `> list(param, `,`) : typ clause* hint*`
    FuncDec(FuncDec),
}

// Spec

pub type Spec = Vec<Def>;
