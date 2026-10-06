//! Prepared SL syntax used by definition environments
//!
//! Shared SL nodes use the prepared stage, with slots and interned mixops.
//! Rule calls select ordinary or tail execution through `lang::sl::prepared`.

use crate::lang::sl::ast as source;

pub use crate::lang::sl::prepared::{Rule, RuleTailInstr};

pub use crate::lang::sl::ast::{DefinedTyp, ExternTyp, RelSignature, TypDef, VarDef};

pub use crate::interp::shared::prepare::ast::*;

// == Prepared syntax

// - Parameters

pub type Param = source::Param<Prepared>;
pub type ParamKind = source::ParamKind<Prepared>;

// - Dangling

pub type Dangle = source::Dangle;

// - Holding conditions

pub type HoldCase = source::HoldCase<Prepared>;

// - Case analysis

pub type Guard = source::Guard<Prepared>;
pub type Case = source::Case<Prepared>;

// - Instructions

pub type Instr = source::Instr<Prepared>;
pub type InstrKind = source::InstrKind<Prepared>;
pub type IfInstr = source::IfInstr<Prepared>;
pub type HoldInstr = source::HoldInstr<Prepared>;
pub type CaseInstr = source::CaseInstr<Prepared>;
pub type GroupInstr = source::GroupInstr<Prepared>;
pub type LetInstr = source::LetInstr<Prepared>;
pub type RuleInstr = source::RuleInstr<Prepared>;
pub type ResultInstr = source::ResultInstr<Prepared>;
pub type ReturnInstr = source::ReturnInstr<Prepared>;
pub type DebugInstr = source::DebugInstr<Prepared>;
pub type InstrIter = PremIter;

// - Blocks

pub type Block = source::Block<Prepared>;
pub type ElseBlock = source::ElseBlock<Prepared>;

// - Table rows

pub type TableRow = source::TableRow<Prepared>;

// - Relation definitions

pub type RelDef = source::RelDef<Prepared>;
pub type ExternRel = source::ExternRel<Prepared>;
pub type DefinedRel = source::DefinedRel<Prepared>;

// - Meta-function definitions

pub type MetaFuncDef = source::MetaFuncDef<Prepared>;
pub type ExternFunc = source::ExternFunc<Prepared>;
pub type BuiltinFunc = source::BuiltinFunc<Prepared>;
pub type TableFunc = source::TableFunc<Prepared>;
pub type DefinedFunc = source::DefinedFunc<Prepared>;
