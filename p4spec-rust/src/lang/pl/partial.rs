//! Partiality checks for prose-language data

use super::ast::*;
use crate::lang::traits::has_call::HasCall;

// == Expressions

/// Reports whether expression evaluation can invoke a fallible call
pub fn is_partial_exp(exp: &Exp) -> bool {
    exp.has_call()
}

// == Cases and guards

/// Reports whether a case guard can invoke a fallible call
pub fn is_partial_case<Tier>(case: &Case<Tier>) -> bool {
    is_partial_guard(&case.guard)
}

/// Reports whether guard evaluation can invoke a fallible call
pub fn is_partial_guard(guard: &Guard) -> bool {
    match guard {
        Guard::Bool(_) | Guard::Sub(..) | Guard::Match(_) | Guard::Mem(_) => false,
        Guard::Cmp(_, _, exp) | Guard::CheckLetSub(_, _, exp) | Guard::CheckLetMatch(_, exp) => {
            is_partial_exp(exp)
        }
    }
}

// == Instructions

/// Reports whether a group-tier instruction can fail before entering nested blocks
pub fn is_partial_group_instr(instr: &GroupInstr) -> bool {
    match instr {
        GroupInstr::Rule(RuleInstr { not_exp, .. }) => {
            not_exp.args().into_iter().any(is_partial_exp)
        }
        GroupInstr::Result(ResultInstr { exps_output, .. }) => {
            exps_output.iter().any(is_partial_exp)
        }
        GroupInstr::Return(ReturnInstr { exp }) => is_partial_exp(exp),
        GroupInstr::Backtrack(_) => false,
    }
}

/// Reports whether an instruction can fail before entering nested blocks
pub fn is_partial_instr<Tier>(
    is_partial_tier: impl Fn(&Tier) -> bool,
    instr: &Instr<Tier>,
) -> bool {
    match &instr.node.node {
        InstrKind::If(IfInstr { exp, .. }) => is_partial_exp(exp),
        InstrKind::Hold(..) => true,
        InstrKind::Case(CaseInstr { exp, cases, .. }) => {
            is_partial_exp(exp) || cases.iter().any(is_partial_case)
        }
        InstrKind::Let(LetInstr { exp_r, .. }) => is_partial_exp(exp_r),
        InstrKind::Debug(DebugInstr { exp }) | InstrKind::Destruct(DestructInstr { exp, .. }) => {
            is_partial_exp(exp)
        }
        InstrKind::CheckLetSub(..) | InstrKind::CheckLetMatch(..) | InstrKind::OptionGet(..) => {
            true
        }
        InstrKind::Tier(TierInstr { tier }) => is_partial_tier(tier),
    }
}
