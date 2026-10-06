//! PL syntax instantiated with execution slots
//!
//! Preparation retains PL expression forms and every prose annotation.
//! Identifiers and iterations resolve to the same slots as AL and SL;
//! evaluation removes expression hints at the shared evaluator boundary.

pub use crate::lang::il::prepared::*;

use super::ast as source;

pub use source::{Fallthrough, RelSignature, TierInstr};

// == Prepared syntax

// - Expressions

pub type Exp = source::Exp<Prepared>;
pub type ExpKind = source::ExpKind<Prepared>;
pub type NotExp = source::NotExp<Prepared>;

// - Paths

pub type Path = source::Path<Prepared>;
pub type PathKind = source::PathKind<Prepared>;

// - Arguments

pub type Arg = source::Arg<Prepared>;
pub type ArgKind = source::ArgKind<Prepared>;

// - Parameters

pub type Param = source::Param<Exp>;
pub type ParamKind = source::ParamKind<Exp>;

// - Holding conditions

pub type HoldCase<Tier> = source::HoldCase<Tier, Exp, Prepared>;

// - Case analysis

pub type Guard = source::Guard<Exp, Prepared>;
pub type Case<Tier> = source::Case<Tier, Exp, Prepared>;

// - Instructions

pub type Instr<Tier> = source::Instr<Tier, Exp, Prepared>;
pub type InstrKind<Tier> = source::InstrKind<Tier, Exp, Prepared>;
pub type IfInstr<Tier> = source::IfInstr<Tier, Exp, Prepared>;
pub type HoldInstr<Tier> = source::HoldInstr<Tier, Exp, Prepared>;
pub type CaseInstr<Tier> = source::CaseInstr<Tier, Exp, Prepared>;
pub type LetInstr = source::LetInstr<Exp, Prepared>;
pub type DebugInstr = source::DebugInstr<Exp>;
pub type DestructInstr = source::DestructInstr<Exp>;
pub type CheckLetSubInstr<Tier> = source::CheckLetSubInstr<Tier, Exp, Prepared>;
pub type CheckLetMatchInstr<Tier> = source::CheckLetMatchInstr<Tier, Exp, Prepared>;
pub type OptionGetInstr<Tier> = source::OptionGetInstr<Tier, Exp, Prepared>;
pub type InstrIter = PremIter;

// - Blocks

pub type Block<Tier> = source::Block<Tier, Exp, Prepared>;
pub type GroupBlock = source::GroupBlock<Exp, Prepared>;
pub type DispatchBlock = source::DispatchBlock<Exp, Prepared>;

// - Group-body tier

pub type GroupInstr = source::GroupInstr<Exp, Prepared>;
pub type ResultInstr = source::ResultInstr<Exp>;
pub type ReturnInstr = source::ReturnInstr<Exp>;
pub type RuleInstr = source::RuleInstr<Exp, Prepared>;
pub type BacktrackInstr = source::BacktrackInstr<Exp, Prepared>;

// - Dispatch tier

pub type DispatchInstr = source::DispatchInstr<Exp, Prepared>;
pub type RuleGroupInstr = source::RuleGroupInstr<Exp, Prepared>;
pub type RouteInstr = source::RouteInstr<Exp, Prepared>;

// - Table rows

pub type TableRow = source::TableRow<Exp, Prepared>;

// - Relation definitions

pub type RelDef = source::RelDef<Exp, Prepared>;
pub type ExternRel = source::ExternRel<Exp>;
pub type DefinedRel = source::DefinedRel<Exp, Prepared>;

// - Meta-function definitions

pub type MetaFuncDef = source::MetaFuncDef<Exp, Prepared>;
pub type ExternFunc = source::ExternFunc<Exp>;
pub type BuiltinFunc = source::BuiltinFunc<Exp>;
pub type TableFunc = source::TableFunc<Exp, Prepared>;
pub type DefinedFunc = source::DefinedFunc<Exp, Prepared>;
