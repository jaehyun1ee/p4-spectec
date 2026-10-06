//! Rule calls selected during SL preparation
//!
//! Ordinary calls bind outputs and execute a continuation.
//! Tail calls carry only their relation and inputs;
//! preparation establishes tail position before discarding the continuation.
//! Other SL instructions keep the shared `ast` representation.

use super::{
    ast::{Exp, Id, RuleInstr},
    stage::Prepared,
};

/// A prepared relation call with its execution form.
#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    /// Binds the relation's outputs and runs its continuation.
    Call(RuleInstr<Prepared>),
    /// Returns the relation's outputs through the tail-call dispatch loop.
    Tail(RuleTailInstr),
}

/// A relation call whose outputs conclude the enclosing relation.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleTailInstr {
    pub id: Id,
    pub exps_input: Vec<Exp<Prepared>>,
}
