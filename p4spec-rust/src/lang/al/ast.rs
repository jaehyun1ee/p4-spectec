//! Algorithmic language model

use crate::{
    domain::source::Spanned,
    lang::{el, hints::input::InputHint, il},
};

// Numbers

pub type Num = il::ast::Num;

// Texts

pub type Text = il::ast::Text;

// Identifiers

pub type Id = il::ast::Id;
pub type IdKind = il::ast::IdKind;

// Atoms

pub type Atom = il::ast::Atom;
pub type AtomKind = il::ast::AtomKind;

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
pub type ExpKind = il::ast::ExpKind;
pub type NotExp = il::ast::NotExp;
pub type IterExp = il::ast::IterExp;

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

pub type Prem = il::ast::Prem;
pub type PremKind = il::ast::PremKind;
pub type IterPrem = il::ast::IterPrem;

// Rules

pub type RuleMatch = (Vec<Exp>, Vec<Exp>, Vec<Prem>);
pub type RulePath = (Id, Vec<Prem>, Vec<Exp>);

pub type RuleGroup = Spanned<RuleGroupKind>;
pub type RuleGroupKind = (Id, RuleMatch, Vec<RulePath>);

pub type ElseGroup = Spanned<ElseGroupKind>;
pub type ElseGroupKind = (Id, RuleMatch, RulePath);

// Clauses

pub type Clause = il::ast::Clause;
pub type ClauseKind = il::ast::ClauseKind;
pub type ElseClause = il::ast::ElseClause;
pub type ElseClauseKind = il::ast::ElseClauseKind;

// Table rows

pub type TableRow = Spanned<TableRowKind>;
pub type TableRowKind = (Vec<Exp>, Vec<Arg>, Exp, Vec<Prem>);

// Hints

pub type Hint = el::ast::Hint;

// Definitions

pub type Def = Spanned<DefKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    /// `extern syntax id hint*`
    ExternTypD(Id, Vec<Hint>),
    /// `syntax id <list(tparam, ,)> = deftyp hint*`
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
    /// `extern dec id <list(tparam, ,)> list(param, ,) : typ hint*`
    ExternDecD(Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>),
    /// `builtin dec id <list(tparam, ,)> list(param, ,) : typ hint*`
    BuiltinDecD(Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>),
    /// `table dec id list(param, ,) : typ tablerow* hint*`
    TableDecD(Id, Vec<Param>, Typ, Vec<TableRow>, Vec<Hint>),
    /// `dec id <list(tparam, ,)> list(param, ,) : typ clause* hint*`
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
