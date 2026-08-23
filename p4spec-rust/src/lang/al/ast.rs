//! Algorithmic language model

use crate::{
    domain::source::Spanned,
    lang::{el, hints::input::T as InputHint, il},
};

pub type Num = il::ast::Num;
pub type Text = il::ast::Text;
pub type Id = il::ast::Id;
pub type IdKind = il::ast::IdKind;
pub type Atom = il::ast::Atom;
pub type AtomKind = il::ast::AtomKind;
pub type Mixop = il::ast::Mixop;
pub type Iter = il::ast::Iter;
pub type Var = il::ast::Var;
pub type Typ = il::ast::Typ;
pub type TypKind = il::ast::TypKind;
pub type NotTyp = il::ast::NotTyp;
pub type NotTypKind = il::ast::NotTypKind;
pub type DefTyp = il::ast::DefTyp;
pub type DefTypKind = il::ast::DefTypKind;
pub type TypField = il::ast::TypField;
pub type TypCase = il::ast::TypCase;
pub type Value = il::ast::Value;
pub type ValueKind = il::ast::ValueKind;
pub type ValueField = il::ast::ValueField;
pub type ValueCase = il::ast::ValueCase;
pub type NumOp = il::ast::NumOp;
pub type UnOp = il::ast::UnOp;
pub type BinOp = il::ast::BinOp;
pub type CmpOp = il::ast::CmpOp;
pub type OpTyp = il::ast::OpTyp;
pub type Subcheck = il::ast::Subcheck;
pub type Exp = il::ast::Exp;
pub type ExpKind = il::ast::ExpKind;
pub type NotExp = il::ast::NotExp;
pub type IterExp = il::ast::IterExp;
pub type Pattern = il::ast::Pattern;
pub type Path = il::ast::Path;
pub type PathKind = il::ast::PathKind;
pub type Param = il::ast::Param;
pub type ParamKind = il::ast::ParamKind;
pub type TParam = il::ast::TParam;
pub type TParamKind = il::ast::TParamKind;
pub type Arg = il::ast::Arg;
pub type ArgKind = il::ast::ArgKind;
pub type Targ = il::ast::Targ;
pub type TargKind = il::ast::TargKind;
pub type Prem = il::ast::Prem;
pub type PremKind = il::ast::PremKind;
pub type IterPrem = il::ast::IterPrem;
pub type Clause = il::ast::Clause;
pub type ClauseKind = il::ast::ClauseKind;
pub type ElseClause = il::ast::ElseClause;
pub type ElseClauseKind = il::ast::ElseClauseKind;

pub type RuleMatch = (Vec<Exp>, Vec<Exp>, Vec<Prem>);
pub type RulePath = (Id, Vec<Prem>, Vec<Exp>);

pub type RuleGroup = Spanned<RuleGroupKind>;
pub type RuleGroupKind = (Id, RuleMatch, Vec<RulePath>);

pub type ElseGroup = Spanned<ElseGroupKind>;
pub type ElseGroupKind = (Id, RuleMatch, RulePath);

pub type TableRow = Spanned<TableRowKind>;
pub type TableRowKind = (Vec<Exp>, Vec<Arg>, Exp, Vec<Prem>);

pub type Hint = el::ast::Hint;

pub type Def = Spanned<DefKind>;

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    ExternTypD(Id, Vec<Hint>),
    TypD(Id, Vec<TParam>, DefTyp, Vec<Hint>),
    VarD(Id, Typ, Vec<Hint>),
    ExternRelD(Id, NotTyp, InputHint, Vec<Hint>),
    RelD(
        Id,
        NotTyp,
        InputHint,
        Vec<RuleGroup>,
        Option<ElseGroup>,
        Vec<Hint>,
    ),
    ExternDecD(Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>),
    BuiltinDecD(Id, Vec<TParam>, Vec<Param>, Typ, Vec<Hint>),
    TableDecD(Id, Vec<Param>, Typ, Vec<TableRow>, Vec<Hint>),
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

pub type Spec = Vec<Def>;
