#![allow(clippy::large_enum_variant)]

use crate::{
    domain::{
        mixfix::Mixfix,
        source::{HasSpan, Span, Spanned},
    },
    lang::{il, pl::annot, sl},
};
pub type Num = sl::ast::Num;
pub type Text = sl::ast::Text;
pub type Id = sl::ast::Id;
pub type Atom = sl::ast::Atom;
pub type Mixop = sl::ast::Mixop;
pub type Iter = sl::ast::Iter;
pub type Var = sl::ast::Var;
pub type Typ = sl::ast::Typ;
pub type TypKind = sl::ast::TypKind;
pub type NotTyp = sl::ast::NotTyp;
pub type NotTypKind = sl::ast::NotTypKind;
pub type DefTyp = sl::ast::DefTyp;
pub type DefTypKind = sl::ast::DefTypKind;
pub type TypField = sl::ast::TypField;
pub type TypCase = sl::ast::TypCase;
pub type Value = sl::ast::Value;
pub type Pattern = sl::ast::Pattern;
pub type UnOp = sl::ast::UnOp;
pub type BinOp = sl::ast::BinOp;
pub type CmpOp = sl::ast::CmpOp;
pub type OpTyp = sl::ast::OpTyp;
pub type Subcheck = il::ast::Subcheck;
pub type TParam = sl::ast::TParam;
pub type Targ = sl::ast::Targ;
pub type IterExp = sl::ast::IterExp;
#[derive(Clone, Debug, PartialEq)]
pub struct ExpNode {
    pub kind: ExpKind,
    pub ty: TypKind,
    pub span: Span,
}
pub type Exp = annot::T<ExpNode>;
impl ExpNode {
    pub fn new(kind: ExpKind, ty: TypKind, span: Span) -> Exp {
        annot::no_hints(Self { kind, ty, span })
    }
}
impl HasSpan for ExpNode {
    fn span(&self) -> &Span {
        &self.span
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum ExpKind {
    BoolE(bool),
    NumE(Num),
    TextE(Text),
    VarE(Id),
    UnE(UnOp, OpTyp, Box<Exp>),
    BinE(BinOp, OpTyp, Box<Exp>, Box<Exp>),
    CmpE(CmpOp, OpTyp, Box<Exp>, Box<Exp>),
    UpCastE(Typ, Box<Exp>),
    DownCastE(Typ, Box<Exp>),
    SubE(Box<Exp>, Typ, Box<Subcheck>),
    MatchE(Box<Exp>, Pattern),
    TupleE(Vec<Exp>),
    CaseE(Box<NotExp>),
    StrE(Vec<(Atom, Exp)>),
    OptE(Option<Box<Exp>>),
    ListE(Vec<Exp>),
    ConsE(Box<Exp>, Box<Exp>),
    CatE(Box<Exp>, Box<Exp>),
    MemE(Box<Exp>, Box<Exp>),
    LenE(Box<Exp>),
    DotE(Box<Exp>, Atom),
    IdxE(Box<Exp>, Box<Exp>),
    SliceE(Box<Exp>, Box<Exp>, Box<Exp>),
    UpdE(Box<Exp>, Box<Path>, Box<Exp>),
    CallE(Id, Vec<Targ>, Vec<Arg>),
    IterE(Box<Exp>, IterExp),
}
pub type NotExp = Mixfix<Exp>;
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub kind: PathKind,
    pub ty: TypKind,
    pub span: Span,
}
impl Path {
    pub fn new(kind: PathKind, ty: TypKind, span: Span) -> Self {
        Self { kind, ty, span }
    }
}
impl HasSpan for Path {
    fn span(&self) -> &Span {
        &self.span
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PathKind {
    RootP,
    IdxP(Box<Path>, Box<Exp>),
    SliceP(Box<Path>, Box<Exp>, Box<Exp>),
    DotP(Box<Path>, Atom),
}
pub type Param = Spanned<ParamKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    ExpP(Typ, Exp),
    DefP(Id, Vec<TParam>, Vec<Param>, Typ),
}
pub type Arg = Spanned<ArgKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    ExpA(Exp),
    DefA(Id),
}
