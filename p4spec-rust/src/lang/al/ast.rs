//! Algorithmic language model

use crate::lang::{common::source::Phrase, el, hints::input::InputHint, il};

// Numbers

pub type Num = il::ast::Num;

// Texts

pub type Text = il::ast::Text;

// Identifiers

pub type Id = il::ast::Id;
pub type IdKind = il::ast::IdKind;

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

pub type Exp = il::ast::Exp;
pub type ExpField = il::ast::ExpField;
pub type ExpKind = il::ast::ExpKind;
pub type NotExp = il::ast::NotExp;
pub type ExpIter = il::ast::ExpIter;

// Patterns

pub type Pattern = il::ast::Pattern;

// Path

pub type Path = il::ast::Path;
pub type PathKind = il::ast::PathKind;

// Parameters

pub type Param = il::ast::Param;
pub type ParamKind = il::ast::ParamKind;

// Type parameters

pub type TParam = il::ast::TParam;
pub type TParamKind = il::ast::TParamKind;

// Arguments

pub type Arg = il::ast::Arg;
pub type ArgKind = il::ast::ArgKind;

// Type arguments

pub type Targ = il::ast::Targ;
pub type TargKind = il::ast::TargKind;

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
pub struct LetPrem {
    pub exp_l: Exp,
    pub exp_r: Exp,
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
    /// `let exp = exp`
    Let(LetPrem),
    /// `prem iterprem`
    Iter(IterPrem),
    /// `debug exp`
    Debug(DebugPrem),
}

pub type PremIter = il::ast::PremIter;

// Rules

#[derive(Clone, Debug, PartialEq)]
pub struct RuleMatch {
    pub exps_signature: Vec<Exp>,
    pub exps_input: Vec<Exp>,
    pub prems: Vec<Prem>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RulePath {
    pub id: Id,
    pub prems: Vec<Prem>,
    pub exps_output: Vec<Exp>,
}

pub type RuleGroup = Phrase<RuleGroupKind>;
#[derive(Clone, Debug, PartialEq)]
pub struct RuleGroupKind {
    pub id: Id,
    pub rule_match: RuleMatch,
    pub rule_paths: Vec<RulePath>,
}

pub type ElseGroup = Phrase<ElseGroupKind>;
#[derive(Clone, Debug, PartialEq)]
pub struct ElseGroupKind {
    pub id: Id,
    pub rule_match: RuleMatch,
    pub rule_path: RulePath,
}

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
#[derive(Clone, Debug, PartialEq)]
pub struct TableRowKind {
    pub exps_signature: Vec<Exp>,
    pub args: Vec<Arg>,
    pub exp: Exp,
    pub prems: Vec<Prem>,
}

// Hints

pub type Hint = el::ast::Hint;

// Definitions

pub type Def = Phrase<DefKind>;

#[derive(Clone, Debug, PartialEq)]
pub struct ExternTypDef {
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
pub struct ExternRelDef {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelDef {
    pub id: Id,
    pub not_typ: NotTyp,
    pub input_hint: InputHint,
    pub rule_groups: Vec<RuleGroup>,
    pub else_group: Option<ElseGroup>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternDecDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinDecDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableDecDef {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub table_rows: Vec<TableRow>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncDecDef {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub clauses: Vec<Clause>,
    pub else_clause: Option<ElseClause>,
    pub hints: Vec<Hint>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    /// `extern syntax id hint*`
    ExternTyp(ExternTypDef),
    /// `syntax id <list(tparam, ,)> = def_typ hint*`
    Typ(TypDef),
    /// `var id : typ hint*`
    Var(VarDef),
    /// `extern relation id : not_typ hint(input %int*) hint*`
    ExternRel(ExternRelDef),
    /// `relation id : not_typ hint(input %int*) rulegroup* hint*`
    Rel(RelDef),
    /// `extern dec id <list(tparam, ,)> list(param, ,) : typ hint*`
    ExternDec(ExternDecDef),
    /// `builtin dec id <list(tparam, ,)> list(param, ,) : typ hint*`
    BuiltinDec(BuiltinDecDef),
    /// `table dec id list(param, ,) : typ tablerow* hint*`
    TableDec(TableDecDef),
    /// `dec id <list(tparam, ,)> list(param, ,) : typ clause* hint*`
    FuncDec(FuncDecDef),
}

// Spec

pub type Spec = Vec<Def>;
