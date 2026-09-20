//! Algorithmic language model

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
pub type ExpField<I = Id, V = Var> = il::ast::ExpField<I, V>;
pub type ExpKind<I = Id, V = Var> = il::ast::ExpKind<I, V>;
pub type NotExp<I = Id, V = Var> = il::ast::NotExp<I, V>;
pub type ExpIter<V = Var> = il::ast::ExpIter<V>;

// Patterns

pub type Pattern = il::ast::Pattern;

// Path

pub type Path<I = Id, V = Var> = il::ast::Path<I, V>;
pub type PathKind<I = Id, V = Var> = il::ast::PathKind<I, V>;

// Parameters

pub type Param = il::ast::Param;
pub type ParamKind = il::ast::ParamKind;

// Type parameters

pub type TParam = il::ast::TParam;

// Arguments

pub type Arg<I = Id, V = Var> = il::ast::Arg<I, V>;
pub type ArgKind<I = Id, V = Var> = il::ast::ArgKind<I, V>;

// Type arguments

pub type Targ = il::ast::Targ;
pub type TargKind = il::ast::TargKind;

// Premises

pub type Prem<I = Id, V = Var> = Phrase<PremKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub struct RulePrem<I = Id, V = Var> {
    pub id: Id,
    pub not_exp: NotExp<I, V>,
    pub input_hint: InputHint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfPrem<I = Id, V = Var> {
    pub exp: Exp<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfHoldPrem<I = Id, V = Var> {
    pub id: Id,
    pub not_exp: NotExp<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IfNotHoldPrem<I = Id, V = Var> {
    pub id: Id,
    pub not_exp: NotExp<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LetPrem<I = Id, V = Var> {
    pub exp_l: Exp<I, V>,
    pub exp_r: Exp<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IterPrem<I = Id, V = Var> {
    pub prem: Box<Prem<I, V>>,
    pub prem_iter: PremIter<V>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugPrem<I = Id, V = Var> {
    pub exp: Exp<I, V>,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum PremKind<I = Id, V = Var> {
    /// `id : notexp`
    Rule(RulePrem<I, V>),
    /// `if exp`
    If(IfPrem<I, V>),
    /// `if id : notexp holds`
    IfHold(IfHoldPrem<I, V>),
    /// `if id : notexp does not hold`
    IfNotHold(IfNotHoldPrem<I, V>),
    /// `let exp = exp`
    Let(LetPrem<I, V>),
    /// `prem iterprem`
    Iter(IterPrem<I, V>),
    /// `debug exp`
    Debug(DebugPrem<I, V>),
}

pub type PremIter<V = Var> = il::ast::PremIter<V>;

// Rules

#[derive(Clone, Debug, PartialEq)]
pub struct RuleMatch<I = Id, V = Var> {
    pub exps_signature: Vec<Exp<I, V>>,
    pub exps_input: Vec<Exp<I, V>>,
    pub prems: Vec<Prem<I, V>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RulePath<I = Id, V = Var> {
    pub id: Id,
    pub prems: Vec<Prem<I, V>>,
    pub exps_output: Vec<Exp<I, V>>,
}

pub type RuleGroup<I = Id, V = Var> = Phrase<RuleGroupKind<I, V>>;
#[derive(Clone, Debug, PartialEq)]
pub struct RuleGroupKind<I = Id, V = Var> {
    pub id: Id,
    pub rule_match: RuleMatch<I, V>,
    pub rule_paths: Vec<RulePath<I, V>>,
}

pub type ElseGroup<I = Id, V = Var> = Phrase<ElseGroupKind<I, V>>;
#[derive(Clone, Debug, PartialEq)]
pub struct ElseGroupKind<I = Id, V = Var> {
    pub id: Id,
    pub rule_match: RuleMatch<I, V>,
    pub rule_path: RulePath<I, V>,
}

// Clauses

pub type Clause<I = Id, V = Var> = Phrase<ClauseKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub struct ClauseKind<I = Id, V = Var> {
    pub args: Vec<Arg<I, V>>,
    pub exp: Exp<I, V>,
    pub prems: Vec<Prem<I, V>>,
}

pub type ElseClause<I = Id, V = Var> = Clause<I, V>;
pub type ElseClauseKind<I = Id, V = Var> = ClauseKind<I, V>;

// Table rows

pub type TableRow<I = Id, V = Var> = Phrase<TableRowKind<I, V>>;
#[derive(Clone, Debug, PartialEq)]
pub struct TableRowKind<I = Id, V = Var> {
    pub exps_signature: Vec<Exp<I, V>>,
    pub args: Vec<Arg<I, V>>,
    pub exp: Exp<I, V>,
    pub prems: Vec<Prem<I, V>>,
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
pub enum RelDef<I = Id, V = Var> {
    /// `extern relation id : not_typ hint(input %int*) hint*`
    Extern(Box<ExternRel>),
    /// `relation id : not_typ hint(input %int*) rulegroup* hint*`
    Defined(Box<DefinedRel<I, V>>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedRel<I = Id, V = Var> {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub rule_groups: Vec<RuleGroup<I, V>>,
    pub else_group: Option<ElseGroup<I, V>>,
    pub hints: Vec<Hint>,
}

// Meta-functions

#[derive(Clone, Debug, PartialEq)]
pub enum MetaFuncDef<I = Id, V = Var> {
    /// `extern dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Extern(ExternFunc),
    /// `builtin dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Builtin(BuiltinFunc),
    /// `table dec id list(param, `,`) : typ hint*`
    Table(TableFunc<I, V>),
    /// `dec id <` list(tparam, `,`) `> list(param, `,`) : typ clause* hint*`
    Defined(Box<DefinedFunc<I, V>>),
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
pub struct TableFunc<I = Id, V = Var> {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub table_rows: Vec<TableRow<I, V>>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedFunc<I = Id, V = Var> {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub clauses: Vec<Clause<I, V>>,
    pub else_clause: Option<ElseClause<I, V>>,
    pub hints: Vec<Hint>,
}

// Definitions

pub type Def<I = Id, V = Var> = Phrase<DefKind<I, V>>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind<I = Id, V = Var> {
    Typ(TypDef),
    /// `var id : typ hint*`
    Var(VarDef),
    Rel(RelDef<I, V>),
    MetaFunc(MetaFuncDef<I, V>),
}

// Spec

pub type Spec<I = Id, V = Var> = Vec<Def<I, V>>;
