//! Partiality checks for algorithmic-language data

use super::ast::*;
use crate::lang::traits::has_call::HasCall;

/// A construct is partial when its evaluation can fail because it invokes a
/// relation or function that may not match
pub fn is_partial_exp(exp: &Exp) -> bool {
    exp.has_call()
}

/// Checks whether a premise may fail during evaluation
pub fn is_partial_prem(prem: &Prem) -> bool {
    match &prem.node {
        PremKind::Rule(_) | PremKind::If(_) | PremKind::IfHold(_) | PremKind::IfNotHold(_) => true,
        PremKind::Let(prem) => is_partial_exp(&prem.exp_r),
        PremKind::Iter(prem) => is_partial_prem(&prem.prem),
        PremKind::Debug(prem) => is_partial_exp(&prem.exp),
    }
}
