use crate::{
    domain::{
        mixfix::Mixfix,
        source::{HasSpan, Span, Spanned},
    },
    lang::{pl::annot, sl},
};

// Numbers

pub type Num = sl::ast::Num;

// Texts

pub type Text = sl::ast::Text;

// Identifiers

pub type Id = sl::ast::Id;

// Atoms

pub type Atom = sl::ast::Atom;

// Mixfix operators

pub type Mixop = sl::ast::Mixop;

// Iterators

pub type Iter = sl::ast::Iter;

// Variables

pub type Var = sl::ast::Var;

// Types

pub type Typ = sl::ast::Typ;
pub type TypKind = sl::ast::TypKind;
pub type NotTyp = sl::ast::NotTyp;
pub type NotTypKind = sl::ast::NotTypKind;
pub type DefTyp = sl::ast::DefTyp;
pub type DefTypKind = sl::ast::DefTypKind;
pub type TypField = sl::ast::TypField;
pub type TypCase = sl::ast::TypCase;

// Values

pub type Value = sl::ast::Value;

// Operators

pub type UnOp = sl::ast::UnOp;
pub type BinOp = sl::ast::BinOp;
pub type CmpOp = sl::ast::CmpOp;
pub type OpTyp = sl::ast::OpTyp;

// Subtype checks

pub type Subcheck = sl::ast::Subcheck;

// Expressions

#[derive(Clone, Debug, PartialEq)]
pub struct ExpNode {
    pub kind: ExpKind,
    pub ty: TypKind,
    pub span: Span,
}
pub type Exp = annot::Annotated<ExpNode>;
impl ExpNode {
    pub fn new(kind: ExpKind, ty: TypKind, span: Span) -> Exp {
        annot::Annotated::new(Self { kind, ty, span })
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
pub type IterExp = sl::ast::IterExp;

// Patterns

pub type Pattern = sl::ast::Pattern;

// Path

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

// Type parameters

pub type TParam = sl::ast::TParam;

// Parameters

pub type Param = Spanned<ParamKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    ExpP(Typ, Box<Exp>),
    DefP(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type arguments

pub type Targ = sl::ast::Targ;

// Arguments

pub type Arg = Spanned<ArgKind>;
#[derive(Clone, Debug, PartialEq)]
pub enum ArgKind {
    ExpA(Box<Exp>),
    DefA(Id),
}

// Dangling

pub type Dangle = sl::ast::Dangle;

// Holding conditions

#[derive(Clone, Debug, PartialEq)]
pub enum HoldCase<Tier> {
    BothH(Block<Tier>, Block<Tier>),
    HoldH(Block<Tier>, Dangle),
    NotHoldH(Block<Tier>, Dangle),
}

// Case analysis

#[derive(Clone, Debug, PartialEq)]
pub struct Case<Tier> {
    pub guard: Guard,
    pub block: Block<Tier>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Guard {
    BoolG(bool),
    CmpG(CmpOp, OpTyp, Exp),
    SubG(Typ, Box<Subcheck>),
    MatchG(Pattern),
    MemG(Exp),
    // Shorthands
    CheckLetSubG(Typ, Box<Subcheck>, Exp),
    CheckLetMatchG(Pattern, Exp),
}

// Instructions

pub type Iid = sl::ast::Iid;

#[derive(Clone, Debug, PartialEq)]
pub enum Fallthrough {
    FallGroup(Id),
    FallNext,
    FallElse,
    FallFail,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InstrNode<Tier> {
    pub kind: InstrKind<Tier>,
    pub iid: Iid,
    pub fallthrough: Option<Fallthrough>,
    pub span: Span,
}

pub type Instr<Tier> = annot::Annotated<InstrNode<Tier>>;

impl<Tier> InstrNode<Tier> {
    pub fn new(
        kind: InstrKind<Tier>,
        iid: Iid,
        fallthrough: Option<Fallthrough>,
        span: Span,
    ) -> Instr<Tier> {
        annot::Annotated::new(Self {
            kind,
            iid,
            fallthrough,
            span,
        })
    }
}

impl<Tier> HasSpan for InstrNode<Tier> {
    fn span(&self) -> &Span {
        &self.span
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstrKind<Tier> {
    // Shared control flow for both tiers
    IfI(Exp, Vec<IterExp>, Block<Tier>, Dangle),
    HoldI(Id, NotExp, Vec<IterExp>, HoldCase<Tier>),
    CaseI(Exp, Vec<Case<Tier>>, Dangle),
    LetI(Exp, Exp, Vec<IterInstr>),
    DebugI(Exp),
    // Shorthands
    DestructI(Vec<(Option<String>, Exp)>, Exp),
    CheckLetSubI(Typ, Box<Subcheck>, Exp, Exp, Block<Tier>),
    CheckLetMatchI(Pattern, Exp, Exp, Block<Tier>),
    OptionGetI(Exp, Exp, Block<Tier>),
    // Tier-specific instruction
    TierI(Tier),
}

pub type Block<Tier> = Vec<Instr<Tier>>;
pub type IterInstr = sl::ast::IterInstr;

// Relations

pub type RelSignature = sl::ast::RelSignature;

// Group-body tier

#[derive(Clone, Debug, PartialEq)]
pub enum InstrGroup {
    ResultI {
        signature: RelSignature,
        outputs: Vec<Exp>,
    },
    ReturnI(Exp),
    RuleI {
        rule_id: Id,
        notation: NotExp,
        input_hint: crate::lang::hints::input::InputHint,
        iterations: Vec<IterInstr>,
    },
    BacktrackI(Vec<BlockGroup>),
}

pub type BlockGroup = Block<InstrGroup>;

// Dispatch tier

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // Dispatch groups retain their flat payload shape
pub enum InstrDispatch {
    GroupI {
        group_id: Id,
        relation_id: Id,
        signature: RelSignature,
        inputs: Vec<Exp>,
        block: BlockGroup,
    },
    RouteI(Vec<BlockDispatch>),
}

pub type BlockDispatch = Block<InstrDispatch>;

// Relations

#[derive(Clone, Debug, PartialEq)]
pub struct ExternRel {
    pub id: Id,
    pub signature: RelSignature,
    pub inputs: Vec<Exp>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rel {
    pub id: Id,
    pub signature: RelSignature,
    pub inputs: Vec<Exp>,
    pub block: BlockDispatch,
    pub else_block: Option<BlockDispatch>,
}

// Functions

#[derive(Clone, Debug, PartialEq)]
pub struct ExternFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub inputs: Vec<Exp>,
    pub expression: Exp,
    pub block: BlockGroup,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableFunc {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DefinedFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub block: BlockGroup,
    pub else_block: Option<BlockGroup>,
}

// Definitions

#[derive(Clone, Debug, PartialEq)]
pub struct DefNode {
    pub kind: DefKind,
    pub span: Span,
}

pub type Def = annot::Annotated<DefNode>;

impl DefNode {
    pub fn new(kind: DefKind, span: Span) -> Def {
        annot::Annotated::new(Self { kind, span })
    }
}

impl HasSpan for DefNode {
    fn span(&self) -> &Span {
        &self.span
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DefKind {
    ExternTypD(Id),
    TypD(Id, Vec<TParam>, DefTyp),
    VarD(Id, Typ),
    ExternRelD(ExternRel),
    RelD(Rel),
    ExternDecD(ExternFunc),
    BuiltinDecD(BuiltinFunc),
    TableDecD(TableFunc),
    FuncDecD(DefinedFunc),
}

// Spec

pub type Spec = Vec<Def>;
