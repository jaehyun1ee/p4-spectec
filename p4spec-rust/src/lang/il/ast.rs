//! Internal language model

use std::rc::Rc;

use crate::lang::{
    common::prim::num,
    common::{
        self,
        notation::{atom, mixfix::Mixfix, mixop},
        source::{NotePhrase, Phrase},
    },
    data, el,
    hints::input::InputHint,
};

// Numbers

pub type Num = num::Number;

// Texts

pub type Text = String;

// Identifiers

pub type Id = common::Id;

// Atoms

pub type Atom = Phrase<atom::Atom>;

// Mixfix operators

pub type Mixop = mixop::Mixop;

// Iterators

pub type Iter = common::Iter;

// Variables

pub type Var = crate::lang::data::var::Var;

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

#[derive(Clone, Debug, PartialEq)]
pub struct TypField {
    pub atom: Atom,
    pub typ: Typ,
}

pub type TypOrigin = Phrase<TypOriginKind>;
#[derive(Clone, Debug, PartialEq)]
pub struct TypOriginKind {
    pub id: Id,
    pub targs: Vec<Targ>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypCase {
    pub not_typ: NotTyp,
    pub typ_origin: TypOrigin,
    pub hints: Vec<Hint>,
}

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

pub type Exp<I = Id, V = Var> = NotePhrase<ExpKind<I, V>, Rc<TypKind>>;

#[derive(Clone, Debug, PartialEq)]
pub enum ExpKind<I = Id, V = Var> {
    /// `bool`
    Bool(bool),
    /// `num`
    Num(Num),
    /// `text`
    Text(Text),
    /// `varid`
    Id(I),
    /// `unop exp`
    Un(UnOp, OpTyp, Box<Exp<I, V>>),
    /// `exp binop exp`
    Bin(BinOp, OpTyp, Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `exp cmpop exp`
    Cmp(CmpOp, OpTyp, Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `exp as typ`
    UpCast(Box<Typ>, Box<Exp<I, V>>),
    /// `exp as typ`
    DownCast(Box<Typ>, Box<Exp<I, V>>),
    /// `exp <: typ`
    Sub(Box<Exp<I, V>>, Box<Typ>, Box<Subcheck>),
    /// `exp matches pattern`
    Match(Box<Exp<I, V>>, Pattern),
    /// `(` exp* `)`
    Tuple(Vec<Exp<I, V>>),
    /// `notexp`
    Case(Box<NotExp<I, V>>),
    /// `{` expfield* `}`
    Str(Vec<ExpField<I, V>>),
    /// `exp?`
    Opt(Option<Box<Exp<I, V>>>),
    /// `[` exp* `]`
    List(Vec<Exp<I, V>>),
    /// `exp :: exp`
    Cons(Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `exp ++ exp`
    Cat(Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `exp <- exp`
    Mem(Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `|` exp `|`
    Len(Box<Exp<I, V>>),
    /// `exp.atom`
    Dot(Box<Exp<I, V>>, Atom),
    /// `exp [` exp `]`
    Idx(Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `exp [` exp `:` exp `]`
    Slice(Box<Exp<I, V>>, Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `exp [` path `=` exp `]`
    Upd(Box<Exp<I, V>>, Box<Path<I, V>>, Box<Exp<I, V>>),
    /// `$id<` targ* `>(` arg* `)`
    Call(Id, Vec<Targ>, Vec<Arg<I, V>>),
    /// `exp iterexp`
    Iter(Box<Exp<I, V>>, ExpIter<V>),
}

pub type NotExp<I = Id, V = Var> = Mixfix<Exp<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub struct ExpField<I = Id, V = Var> {
    pub atom: Atom,
    pub exp: Exp<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExpIter<V = Var> {
    pub iter: Iter,
    pub vars: Vec<V>,
}

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
    Fixed(usize),
    Nil,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptPattern {
    Some,
    None,
}

// Paths

pub type Path<I = Id, V = Var> = NotePhrase<PathKind<I, V>, Rc<TypKind>>;

#[derive(Clone, Debug, PartialEq)]
pub enum PathKind<I = Id, V = Var> {
    Root,
    /// `path [` exp `]`
    Idx(Box<Path<I, V>>, Box<Exp<I, V>>),
    /// `path [` exp `:` exp `]`
    Slice(Box<Path<I, V>>, Box<Exp<I, V>>, Box<Exp<I, V>>),
    /// `path . atom`
    Dot(Box<Path<I, V>>, Atom),
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

// Arguments

pub type Arg<I = Id, V = Var> = Phrase<ArgKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind<I = Id, V = Var> {
    /// `exp`
    Exp(Box<Exp<I, V>>),
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
pub struct PremIter<V = Var> {
    pub iter: Iter,
    pub vars_bound: Vec<V>,
    pub vars_bind: Vec<V>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct RuleGroupKind {
    pub id: Id,
    pub rules: Vec<Rule>,
}

pub type ElseGroup = Phrase<ElseGroupKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct ElseGroupKind {
    pub id: Id,
    pub rule: Rule,
}

// Clauses

pub type Clause = Phrase<ClauseKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct ClauseKind {
    pub args: Vec<Arg>,
    pub exp: Exp,
    pub prems: Vec<Prem>,
}

pub type ElseClause = Clause;
pub type ElseClauseKind = ClauseKind;

// Table rows

pub type TableRow = Phrase<TableRowKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct TableRowKind {
    pub args: Vec<Arg>,
    pub exp: Exp,
}

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
pub enum RelDef {
    /// `extern relation id : not_typ hint(input %int*) hint*`
    Extern(Box<ExternRel>),
    /// `relation id : not_typ hint(input %int*) rulegroup* hint*`
    Defined(Box<DefinedRel>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedRel {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub rule_groups: Vec<RuleGroup>,
    pub else_group: Option<ElseGroup>,
    pub hints: Vec<Hint>,
}

// Meta-functions

#[derive(Clone, Debug, PartialEq)]
pub enum MetaFuncDef {
    /// `extern dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Extern(ExternFunc),
    /// `builtin dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Builtin(BuiltinFunc),
    /// `table dec id list(param, `,`) : typ hint*`
    Table(TableFunc),
    /// `dec id <` list(tparam, `,`) `> list(param, `,`) : typ clause* hint*`
    Defined(Box<DefinedFunc>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableFunc {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub rows: Vec<TableRow>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub clauses: Vec<Clause>,
    pub else_clause: Option<ElseClause>,
    pub hints: Vec<Hint>,
}

// Definitions

pub type Def = Phrase<DefKind>;

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum DefKind {
    Typ(TypDef),
    /// `var id : typ hint*`
    Var(VarDef),
    Rel(RelDef),
    MetaFunc(MetaFuncDef),
}

// Spec

pub type Spec = Vec<Def>;
