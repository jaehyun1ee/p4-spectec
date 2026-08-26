//! Intermediate language model

use crate::domain::{
    atom,
    mixfix::Mixfix,
    mixop,
    noted::Noted,
    source::{Span, Spanned},
};
use crate::lang::{el, hints::input::InputHint, xl::num};
use crate::yojson::ExternalData;

// Numbers

pub type Num = num::Number;

// Texts

pub type Text = String;

// Identifiers

pub type Id = Spanned<IdKind>;
pub type IdKind = String;

// Atoms

pub type Atom = Spanned<atom::Atom>;

// Mixfix operators

pub type Mixop = mixop::Mixop;

// Iterators

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Iter {
    /// `?`
    Opt,
    /// `*`
    List,
}

// Variables

#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    pub id: Id,
    pub typ: Typ,
    pub iters: Vec<Iter>,
}

// Types

pub type Typ = Spanned<TypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum TypKind {
    /// `bool`
    Bool,
    /// `numtyp`
    Num(num::Typ),
    /// `text`
    Text,
    /// `id (`<` list(targ, `,`) `>`)?`
    Var(Id, Vec<Targ>),
    /// `(` list(typ, `,`) `)`
    Tuple(Vec<Typ>),
    /// `typ iter`
    Iter(Box<Typ>, Iter),
    /// `<` list(tparam, `,`) `>` `(` list(typ, `,`) `)` `:` typ
    Func(Vec<TParam>, Vec<Typ>, Box<Typ>),
}

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

pub type NotTyp = Spanned<NotTypKind>;
pub type NotTypKind = Mixfix<Typ>;

pub type DefTyp = Spanned<DefTypKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefTypKind {
    Plain(Typ),
    Struct(Vec<TypField>),
    Variant(Vec<TypCase>),
}

pub type TypField = (Atom, Typ);
pub type TypOrigin = Spanned<TypOriginKind>;
pub type TypOriginKind = (Id, Vec<Targ>);
pub type TypCase = (NotTyp, TypOrigin, Vec<Hint>);

// Values

pub type Value = Spanned<Noted<ValueKind, TypKind>>;

/// Constructs a typed value.
pub fn value(kind: ValueKind, typ: TypKind, span: Span) -> Value {
    Spanned::new(Noted::new(kind, typ), span)
}

#[derive(Clone, Debug, PartialEq)]
pub enum ValueKind {
    Bool(bool),
    Num(Num),
    Text(Text),
    Struct(Vec<ValueField>),
    Case(Box<ValueCase>),
    Tuple(Vec<Value>),
    Opt(Option<Box<Value>>),
    List(Vec<Value>),
    Func(Id),
    Extern(ExternalData),
}

pub type ValueField = (Atom, Value);
pub type ValueCase = Mixfix<Value>;

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

pub type Exp = Spanned<Noted<ExpKind, TypKind>>;

/// Constructs a typed expression.
pub fn exp(kind: ExpKind, typ: TypKind, span: Span) -> Exp {
    Spanned::new(Noted::new(kind, typ), span)
}

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
    UpCast(Typ, Box<Exp>),
    /// `exp as typ`
    DownCast(Typ, Box<Exp>),
    /// `exp <: typ`
    Sub(Box<Exp>, Typ, Box<Subcheck>),
    /// `exp matches pattern`
    Match(Box<Exp>, Pattern),
    /// `(` exp* `)`
    Tuple(Vec<Exp>),
    /// `notexp`
    Case(Box<NotExp>),
    /// `{` expfield* `}`
    Str(Vec<(Atom, Exp)>),
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
    Upd(Box<Exp>, Path, Box<Exp>),
    /// `$id<` targ* `>(` arg* `)`
    Call(Id, Vec<Targ>, Vec<Arg>),
    /// `exp iterexp`
    Iter(Box<Exp>, IterExp),
}

pub type NotExp = Mixfix<Exp>;
pub type IterExp = (Iter, Vec<Var>);

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

pub type Path = Spanned<Noted<PathKind, TypKind>>;

/// Constructs a typed path.
pub fn path(kind: PathKind, typ: TypKind, span: Span) -> Path {
    Spanned::new(Noted::new(kind, typ), span)
}

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

pub type Param = Spanned<ParamKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    /// `typ`
    Exp(Typ),
    /// `def $id (<` list(tparam, `,`) `>)? (` list(param, `,`) `)? : typ`
    Def(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type parameters

pub type TParam = Spanned<TParamKind>;
pub type TParamKind = IdKind;

// Arguments

pub type Arg = Spanned<ArgKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    /// `exp`
    Exp(Box<Exp>),
    /// `$id`
    Def(Id),
}

// Type arguments

pub type Targ = Spanned<TargKind>;
pub type TargKind = TypKind;

// Premises

pub type Prem = Spanned<PremKind>;

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
pub struct LetPrem {
    pub exp_l: Exp,
    pub exp_r: Exp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IteratedPrem {
    pub prem: Box<Prem>,
    pub iter_prem: IterPrem,
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
    /// `let exp = exp`
    Let(LetPrem),
    /// `prem iterprem`
    Iter(IteratedPrem),
    /// `debug exp`
    Debug(DebugPrem),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IterPrem {
    pub iter: Iter,
    pub vars_bound: Vec<Var>,
    pub vars_bind: Vec<Var>,
}

// Rules

pub type Rule = Spanned<RuleKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct RuleKind {
    pub id: Id,
    pub not_exp: NotExp,
    pub prems: Vec<Prem>,
}

pub type RuleGroup = Spanned<RuleGroupKind>;
pub type RuleGroupKind = (Id, Vec<Rule>);

pub type ElseGroup = Spanned<ElseGroupKind>;
pub type ElseGroupKind = (Id, Rule);

// Clauses

pub type Clause = Spanned<ClauseKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct ClauseKind {
    pub args: Vec<Arg>,
    pub expression: Exp,
    pub premises: Vec<Prem>,
}

pub type ElseClause = Clause;
pub type ElseClauseKind = ClauseKind;

// Table rows

pub type TableRow = Spanned<TableRowKind>;
pub type TableRowKind = (Vec<Arg>, Exp);

// Hints

pub type Hint = el::ast::Hint;

// Definitions

pub type Def = Spanned<DefKind>;

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
