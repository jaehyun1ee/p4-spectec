//! Prose language model
//!
//! Types, values, operators, and patterns are re-exported from SL.
//! Expressions and definitions are `Annotated` so prose hints can attach;
//! instructions carry an optional `Fallthrough` note saying where control goes
//! when they do not conclude.
//! Instructions are generic over a `Tier`, the instruction kind it alone has:
//! `DispatchInstr` selects a rule group, `GroupInstr` runs its body.

use crate::lang::{
    common::{
        notation::mixfix::Mixfix,
        source::{NotePhrase, Phrase},
    },
    pl::annot,
    sl,
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

/// A typed expression before annotation.
pub type ExpNode = NotePhrase<ExpKind, TypKind>;
/// A typed expression with prose hints.
pub type Exp = annot::Annotated<ExpNode>;
#[derive(Clone, Debug, PartialEq)]
/// The forms of an expression, as in SL.
pub enum ExpKind {
    Bool(bool),
    Num(Num),
    Text(Text),
    Id(Id),
    Un(UnOp, OpTyp, Box<Exp>),
    Bin(BinOp, OpTyp, Box<Exp>, Box<Exp>),
    Cmp(CmpOp, OpTyp, Box<Exp>, Box<Exp>),
    UpCast(Typ, Box<Exp>),
    DownCast(Typ, Box<Exp>),
    Sub(Box<Exp>, Typ, Box<Subcheck>),
    Match(Box<Exp>, Pattern),
    Tuple(Vec<Exp>),
    Case(Box<NotExp>),
    Str(Vec<(Atom, Exp)>),
    Opt(Option<Box<Exp>>),
    List(Vec<Exp>),
    Cons(Box<Exp>, Box<Exp>),
    Cat(Box<Exp>, Box<Exp>),
    Mem(Box<Exp>, Box<Exp>),
    Len(Box<Exp>),
    Dot(Box<Exp>, Atom),
    Idx(Box<Exp>, Box<Exp>),
    Slice(Box<Exp>, Box<Exp>, Box<Exp>),
    Upd(Box<Exp>, Box<Path>, Box<Exp>),
    Call(Id, Vec<Targ>, Vec<Arg>),
    Iter(Box<Exp>, ExpIter),
}
/// A notation expression: a mixfix skeleton with expressions as arguments.
pub type NotExp = Mixfix<Exp>;
pub type ExpIter = sl::ast::ExpIter;

// Patterns

pub type Pattern = sl::ast::Pattern;

// Path

/// A typed path into a value, for updates.
pub type Path = NotePhrase<PathKind, TypKind>;
#[derive(Clone, Debug, PartialEq)]
/// The steps of a path, from the root outward.
pub enum PathKind {
    Root,
    Idx(Box<Path>, Box<Exp>),
    Slice(Box<Path>, Box<Exp>, Box<Exp>),
    Dot(Box<Path>, Atom),
}

// Type parameters

pub type TParam = sl::ast::TParam;

// Parameters

/// A function parameter with its span.
pub type Param = Phrase<ParamKind>;
#[derive(Clone, Debug, PartialEq)]
/// A parameter: a typed pattern, or a function with its own signature.
pub enum ParamKind {
    Exp(Typ, Box<Exp>),
    Def(Id, Vec<TParam>, Vec<Param>, Typ),
}

// Type arguments

pub type Targ = sl::ast::Targ;

// Arguments

/// A call argument with its span.
pub type Arg = Phrase<ArgKind>;
#[derive(Clone, Debug, PartialEq)]
/// An argument: a value expression or a function name.
pub enum ArgKind {
    Exp(Box<Exp>),
    Def(Id),
}

// Dangling

pub type Dangle = sl::ast::Dangle;

// Holding conditions

#[derive(Clone, Debug, PartialEq)]
/// Which branches a hold instruction has: both, or one that may dangle.
pub enum HoldCase<Tier> {
    /// The holds branch, then the does-not-hold branch.
    Both(Block<Tier>, Block<Tier>),
    /// Only the holds branch.
    Hold(Block<Tier>, Dangle),
    /// Only the does-not-hold branch.
    NotHold(Block<Tier>, Dangle),
}

// Case analysis

#[derive(Clone, Debug, PartialEq)]
/// One arm of a case analysis: a guard and its block.
pub struct Case<Tier> {
    pub guard: Guard,
    pub block: Block<Tier>,
}

#[derive(Clone, Debug, PartialEq)]
/// A test on the case scrutinee; the shorthands also bind it.
pub enum Guard {
    /// The scrutinee is this boolean.
    Bool(bool),
    /// The scrutinee compares so with the expression.
    Cmp(CmpOp, OpTyp, Exp),
    /// The scrutinee has the type.
    Sub(Typ, Box<Subcheck>),
    /// The scrutinee matches the pattern.
    Match(Pattern),
    /// The scrutinee is an element of the list.
    Mem(Exp),
    // Shorthands
    /// Let the expression be the scrutinee, which has the type.
    CheckLetSub(Typ, Box<Subcheck>, Exp),
    /// Let the expression be the scrutinee, which matches the pattern.
    CheckLetMatch(Pattern, Exp),
}

// Instructions

#[derive(Clone, Debug, PartialEq)]
/// Where control goes when an instruction does not conclude.
pub enum Fallthrough {
    /// To the named rule group.
    Group(Id),
    /// To the next alternative.
    Next,
    /// To the otherwise block.
    Else,
    /// Nowhere: evaluation fails.
    Fail,
}

/// An instruction with its fall-through note, before annotation.
pub type InstrNode<Tier> = NotePhrase<InstrKind<Tier>, Option<Fallthrough>>;
/// An instruction with prose hints.
pub type Instr<Tier> = annot::Annotated<InstrNode<Tier>>;

#[derive(Clone, Debug, PartialEq)]
/// The control flow shared by both tiers, plus the tier's own instruction.
pub enum InstrKind<Tier> {
    /// Run the block if a condition holds.
    If(IfInstr<Tier>),
    /// Run a branch by whether a relation applies.
    Hold(HoldInstr<Tier>),
    /// Run the first arm whose guard accepts.
    Case(CaseInstr<Tier>),
    /// Bind a pattern.
    Let(LetInstr),
    /// Print an expression.
    Debug(DebugInstr),
    /// Shorthand: bind several fields of one value at once.
    Destruct(DestructInstr),
    /// Shorthand: bind after a subtype check, then run the block.
    CheckLetSub(CheckLetSubInstr<Tier>),
    /// Shorthand: bind after a pattern match, then run the block.
    CheckLetMatch(CheckLetMatchInstr<Tier>),
    /// Shorthand: bind the content of a present option, then run the block.
    OptionGet(OptionGetInstr<Tier>),
    /// The tier's own instruction.
    Tier(TierInstr<Tier>),
}

#[derive(Clone, Debug, PartialEq)]
/// Run the block when the condition holds under its iterations.
pub struct IfInstr<Tier> {
    pub exp: Exp,
    pub iter_exps: Vec<ExpIter>,
    pub block: Block<Tier>,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
/// Run a branch by whether the relation applies under its iterations.
pub struct HoldInstr<Tier> {
    pub id: Id,
    pub not_exp: NotExp,
    pub iter_exps: Vec<ExpIter>,
    pub hold_case: HoldCase<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
/// Case analysis on an expression.
pub struct CaseInstr<Tier> {
    pub exp: Exp,
    pub cases: Vec<Case<Tier>>,
    pub dangle: Dangle,
}
#[derive(Clone, Debug, PartialEq)]
/// Bind `exp_l` to `exp_r` under the iterations.
pub struct LetInstr {
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub iter_instrs: Vec<InstrIter>,
}
#[derive(Clone, Debug, PartialEq)]
/// Print the expression.
pub struct DebugInstr {
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
/// Bind each field expression, named when shown, from `exp`.
pub struct DestructInstr {
    pub bindings: Vec<(Option<String>, Exp)>,
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
/// Bind `exp_l` to `exp_r` once it passes the subtype check, then the block.
pub struct CheckLetSubInstr<Tier> {
    pub typ: Typ,
    pub subcheck: Box<Subcheck>,
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub block: Block<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
/// Bind `exp_l` to `exp_r` once it matches the pattern, then run the block.
pub struct CheckLetMatchInstr<Tier> {
    pub pattern: Pattern,
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub block: Block<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
/// Bind `exp_l` to the content of the option `exp_r`, then run the block.
pub struct OptionGetInstr<Tier> {
    pub exp_l: Exp,
    pub exp_r: Exp,
    pub block: Block<Tier>,
}
#[derive(Clone, Debug, PartialEq)]
/// The tier-specific instruction.
pub struct TierInstr<Tier> {
    pub tier: Tier,
}

/// Instructions run in order.
pub type Block<Tier> = Vec<Instr<Tier>>;
pub type InstrIter = sl::ast::InstrIter;

// Relations

pub type RelSignature = sl::ast::RelSignature;

// Group-body tier

#[derive(Clone, Debug, PartialEq)]
/// Instructions of a rule group's body.
pub enum GroupInstr {
    /// Conclude the relation with outputs.
    Result(ResultInstr),
    /// Conclude the function with a value.
    Return(ReturnInstr),
    /// Call a relation and bind its outputs.
    Rule(RuleInstr),
    /// Try the arms in order until one concludes.
    Backtrack(BacktrackInstr),
}

#[derive(Clone, Debug, PartialEq)]
/// The relation's outputs.
pub struct ResultInstr {
    pub rel_signature: RelSignature,
    pub exps_output: Vec<Exp>,
}
#[derive(Clone, Debug, PartialEq)]
/// The function's result.
pub struct ReturnInstr {
    pub exp: Exp,
}
#[derive(Clone, Debug, PartialEq)]
/// A relation call under its iterations; the hint marks the input arguments.
pub struct RuleInstr {
    pub id: Id,
    pub not_exp: NotExp,
    pub input_hint: crate::lang::hints::input::InputHint,
    pub iter_instrs: Vec<InstrIter>,
}
#[derive(Clone, Debug, PartialEq)]
/// Alternative blocks; the first that concludes wins.
pub struct BacktrackInstr {
    pub blocks: Vec<GroupBlock>,
}

/// A block of the group-body tier.
pub type GroupBlock = Block<GroupInstr>;

// Dispatch tier

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
/// Instructions of a relation's dispatch: which group runs.
pub enum DispatchInstr {
    /// Match the inputs against a rule group and run its body.
    Group(RuleGroupInstr),
    /// Try alternative dispatch blocks in order.
    Route(RouteInstr),
}

#[derive(Clone, Debug, PartialEq)]
/// One rule group: its relation, name, input patterns, and body.
pub struct RuleGroupInstr {
    pub id_rel: Id,
    pub id_group: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
    pub block: GroupBlock,
}
#[derive(Clone, Debug, PartialEq)]
/// Alternative dispatch blocks; the first that concludes wins.
pub struct RouteInstr {
    pub blocks: Vec<DispatchBlock>,
}

/// A block of the dispatch tier.
pub type DispatchBlock = Block<DispatchInstr>;

// Type definitions

#[derive(Clone, Debug, PartialEq)]
/// A type definition: extern or defined.
pub enum TypDef {
    /// `extern syntax id hint*`
    Extern(ExternTyp),
    /// `syntax id <` list(tparam, `,`) `> : typ hint*`
    Defined(Box<DefinedTyp>),
}

#[derive(Clone, Debug, PartialEq)]
/// A type defined outside the specification.
pub struct ExternTyp {
    pub id: Id,
}

#[derive(Clone, Debug, PartialEq)]
/// A type with parameters and a body.
pub struct DefinedTyp {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub def_typ: DefTyp,
}

// Meta-variables

#[derive(Clone, Debug, PartialEq)]
/// A meta-variable naming a type.
pub struct VarDef {
    pub id: Id,
    pub typ: Typ,
}

// Relations

#[derive(Clone, Debug, PartialEq)]
/// A relation definition: extern or defined.
pub enum RelDef {
    /// `extern relation id : not_typ hint(input %int*) hint*`
    Extern(ExternRel),
    /// `relation id : not_typ hint(input %int*) rulegroup* hint*`
    Defined(DefinedRel),
}

#[derive(Clone, Debug, PartialEq)]
/// A relation provided by the host.
pub struct ExternRel {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
}

#[derive(Clone, Debug, PartialEq)]
/// A relation as a dispatch block with an optional otherwise block.
pub struct DefinedRel {
    pub id: Id,
    pub rel_signature: RelSignature,
    pub exps_input: Vec<Exp>,
    pub block: DispatchBlock,
    pub block_else_opt: Option<DispatchBlock>,
}

// Meta-functions

#[derive(Clone, Debug, PartialEq)]
/// A function definition: extern, builtin, table, or defined.
pub enum MetaFuncDef {
    /// `extern dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Extern(ExternFunc),
    /// `builtin dec id <` list(tparam, `,`) `> list(param, `,`) : typ hint*`
    Builtin(BuiltinFunc),
    /// `table dec id list(param, `,`) : typ hint*`
    Table(TableFunc),
    /// `dec id <` list(tparam, `,`) `> list(param, `,`) : typ clause* hint*`
    Defined(DefinedFunc),
}

#[derive(Clone, Debug, PartialEq)]
/// A function provided by the host.
pub struct ExternFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
/// A function provided by the interpreter.
pub struct BuiltinFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
}

#[derive(Clone, Debug, PartialEq)]
/// One row: input patterns, the matched expression, and a group-body block.
pub struct TableRow {
    pub exps_input: Vec<Exp>,
    pub exp: Exp,
    pub block: GroupBlock,
}

#[derive(Clone, Debug, PartialEq)]
/// A function defined by table rows.
pub struct TableFunc {
    pub id: Id,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub rows: Vec<TableRow>,
}

#[derive(Clone, Debug, PartialEq)]
/// A function as a group-body block with an optional otherwise block.
pub struct DefinedFunc {
    pub id: Id,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub typ: Typ,
    pub block: GroupBlock,
    pub block_else_opt: Option<GroupBlock>,
}

// Definitions

/// A definition before annotation.
pub type DefNode = Phrase<DefKind>;
/// A definition with prose hints.
pub type Def = annot::Annotated<DefNode>;

#[derive(Clone, Debug, PartialEq)]
/// The forms of a definition.
pub enum DefKind {
    Typ(TypDef),
    Var(VarDef),
    Rel(RelDef),
    MetaFunc(MetaFuncDef),
}

// Spec

/// A whole specification: its definitions in source order.
pub type Spec = Vec<Def>;
