//! Fallthrough anchors and labels for prose-language rendering
//!
//! A renderer owns the counters so each complete document starts from one.
//! Nested backtracking blocks share that state and derive arm targets from it.

use std::collections::BTreeMap;

use crate::lang::pl::ast::{Fallthrough, Instr};

use super::doc::{FallthroughLabel, Prose};

// == Context

/// The fallthrough destination visible while rendering one instruction.
#[derive(Clone, Debug)]
pub struct Context {
    pub namespace: String,
    pub next: Option<String>,
}

/// Per-document counters for backtracking blocks.
#[derive(Default)]
pub struct Anchors {
    block_counters: BTreeMap<String, usize>,
}

impl Anchors {
    /// Allocates the next block anchor in a namespace.
    pub fn fresh_block(&mut self, namespace: &str) -> String {
        let num_blocks = self.block_counters.entry(namespace.to_owned()).or_default();
        *num_blocks += 1;
        format!("bk-{namespace}-{num_blocks}")
    }
}

/// Returns the anchor of an ordered arm.
pub fn anchor_of_arm(anchor_block: &str, idx: usize) -> String {
    format!("{anchor_block}-arm-{}", idx + 1)
}

/// Returns a rule-group anchor, sanitizing path separators.
pub fn anchor_of_group(namespace: &str, id_group: &str) -> String {
    format!("{}-{}", namespace.replace('/', "-"), id_group.replace('/', "-"))
}

/// Returns the anchor of an otherwise block.
pub fn anchor_of_else(namespace: &str) -> String {
    format!("{namespace}-else")
}

/// Renders an instruction's fallthrough marker.
pub fn prose_of_link<Tier>(ctx: &Context, instr: &Instr<Tier>) -> Prose {
    match &instr.node.note {
        None => Prose::Empty,
        Some(Fallthrough::Next) => Prose::Fallthrough(
            ctx.next
                .clone()
                .expect("Fallthrough::Next has a target arm"),
            FallthroughLabel::Derived,
        ),
        Some(Fallthrough::Group(id_group)) => Prose::Fallthrough(
            anchor_of_group(&ctx.namespace, &id_group.node),
            FallthroughLabel::Explicit(id_group.node.clone()),
        ),
        Some(Fallthrough::Else) => Prose::Fallthrough(
            anchor_of_else(&ctx.namespace),
            FallthroughLabel::Explicit("⋅".to_owned()),
        ),
        Some(Fallthrough::Fail) => {
            Prose::Text("+++<sub class=\"bk-mark\">[FAIL]</sub>+++".to_owned())
        }
    }
}
