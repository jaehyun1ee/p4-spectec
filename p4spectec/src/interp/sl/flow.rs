//! SL continuations, returns, relation results, and tail calls
//!
//! `Flow` is what evaluating an instruction yields:
//! `Cont` fell through, carrying the failures collected so far;
//! `Return` and `Result` finish a function or relation;
//! `TailFunc` and `TailRel` ask the invoker to call again in place.
//! `choose_sequential` takes the first non-continuing instruction;
//! `choose_deterministic` runs all and rejects two that terminate.

use std::rc::Rc;

use crate::lang::{
    common::source::{Phrase, Span},
    data::value::Value,
};

use crate::diagnostic::{Diagnostic, Label, Report};

use crate::runtime::envs::interp::sl::ast_prepared as ast;

use crate::interp::shared::{
    backtrack::{Backtrack, fatal, ok, unmatch, unwrap},
    error,
};

/// The outcome of evaluating an instruction or block.
#[derive(Debug)]
pub enum Flow {
    /// Fell through, with the failures met so far.
    Cont(Vec<Report>),
    /// A function body returned a value.
    Return(Phrase<Value>),
    /// A relation body produced its outputs.
    Result(Phrase<Vec<Value>>),
    /// A function call to make in place of the current one.
    TailFunc(Phrase<(ast::Id, Vec<ast::Typ>, Vec<Value>)>),
    /// A relation call to make in place of the current one.
    TailRel(Phrase<(ast::Id, Vec<Value>)>),
}

impl Flow {
    /// Returns the instruction location for a terminating flow.
    fn span(&self) -> &Span {
        match self {
            Self::Return(value) => &value.span,
            Self::Result(values) => &values.span,
            Self::TailFunc(call) => &call.span,
            Self::TailRel(call) => &call.span,
            Self::Cont(_) => unreachable!("continuations have no terminal instruction"),
        }
    }

    // = Continuation

    /// A continuation carrying one premise failure.
    pub(crate) fn cont(span: Span, error: Diagnostic) -> Self {
        let diagnostic = error.with_label(Label::primary(&span, ""));
        Self::Cont(vec![Report::from(diagnostic)])
    }
}

// = Pending condition reports

/// Retains condition text until a continuation crosses a public boundary.
pub(super) enum PendingFlow {
    Eager(Flow),
    Condition { span: Span, text: Rc<str>, case: bool },
}

impl PendingFlow {
    /// Materializes independent public reports for a surviving condition.
    pub(super) fn into_flow(self) -> Flow {
        match self {
            Self::Eager(flow) => flow,
            Self::Condition { span, text, case } => {
                let diagnostic = if case {
                    error::prem::condition_unmet_display(format_args!("case {text}"))
                } else {
                    error::prem::condition_unmet_display(text)
                };
                Flow::cont(span, diagnostic)
            }
        }
    }

    /// Identifies fallthrough without constructing its reports.
    pub(super) fn is_cont(&self) -> bool {
        matches!(self, Self::Eager(Flow::Cont(_)) | Self::Condition { .. })
    }

    /// Converts mismatches at binding and terminal instructions.
    pub(super) fn cont_from_unmatch(result: Backtrack<Self>) -> Backtrack<Self> {
        match result {
            unmatch!(errors) => ok!(Self::Eager(Flow::Cont(errors))),
            result => result,
        }
    }

    /// Measures existing reports or the single childless pending cause.
    fn depth_max(&self) -> usize {
        match self {
            Self::Eager(Flow::Cont(errors)) => {
                errors.iter().map(Report::depth_max).max().unwrap_or(0)
            }
            Self::Condition { .. } => 1,
            Self::Eager(_) => unreachable!("only continuing flows have report depth"),
        }
    }

    /// Keeps the most specific continuation, preferring later equal depths.
    fn retain_deepest(&mut self, flow_post: Self) {
        if flow_post.depth_max() >= self.depth_max() {
            *self = flow_post;
        }
    }
}

// = Sequential choice

/// Tries instructions in order; the last one runs in tail position.
pub(super) fn choose_sequential<C>(
    mut candidates: impl DoubleEndedIterator<Item = C>,
    mut evaluate: impl FnMut(C, bool) -> Backtrack<PendingFlow>,
) -> Backtrack<PendingFlow> {
    // An empty block continues with no failures
    let Some(candidate_last) = candidates.next_back() else {
        return ok!(PendingFlow::Eager(Flow::Cont(vec![])));
    };
    // Non-tail instructions run first; the first one that terminates wins
    let mut flow = PendingFlow::Eager(Flow::Cont(vec![]));
    for candidate in candidates {
        let flow_post = unwrap!(evaluate(candidate, false));
        if flow_post.is_cont() {
            flow.retain_deepest(flow_post);
        } else {
            return ok!(flow_post);
        }
    }
    // The last instruction gets the tail flag
    let flow_post = unwrap!(evaluate(candidate_last, true));
    if flow_post.is_cont() {
        flow.retain_deepest(flow_post);
        ok!(flow)
    } else {
        ok!(flow_post)
    }
}

// = Deterministic choice

/// Merges public flows, preserving conclusion checks and diagnostic ordering.
fn combine_eager(flow: Flow, flow_post: Flow) -> Backtrack<Flow> {
    let flow = match (flow, flow_post) {
        // Both continue: merge the failures
        (Flow::Cont(mut errors), Flow::Cont(errors_post)) => {
            errors.extend(errors_post);
            Flow::Cont(errors)
        }
        // One terminated: keep it
        (Flow::Cont(_), flow) | (flow, Flow::Cont(_)) => flow,
        // Structuring preserves the conclusion kind of each callable
        (Flow::Return(_) | Flow::TailFunc(..), Flow::Result(_) | Flow::TailRel(..))
        | (Flow::Result(_) | Flow::TailRel(..), Flow::Return(_) | Flow::TailFunc(..)) => {
            unreachable!("function and relation conclusions cannot mix")
        }
        // Two conclusions from the same callable are nondeterministic
        (flow, flow_post) => {
            return fatal!(
                *flow_post.span(),
                error::call::instruction_nondeterministic(flow.span()),
            );
        }
    };
    ok!(flow)
}

/// Keeps pending leaves through empty merges until a conclusion wins.
fn combine_deterministic(flow: PendingFlow, flow_post: PendingFlow) -> Backtrack<PendingFlow> {
    // Empty continuations add no reports to a pending leaf
    match (&flow, &flow_post) {
        (PendingFlow::Eager(Flow::Cont(errors)), PendingFlow::Condition { .. })
            if errors.is_empty() =>
        {
            return ok!(flow_post);
        }
        (PendingFlow::Condition { .. }, PendingFlow::Eager(Flow::Cont(errors)))
            if errors.is_empty() =>
        {
            return ok!(flow);
        }
        // A terminal flow wins over a pending condition
        (PendingFlow::Condition { .. }, _) if !flow_post.is_cont() => return ok!(flow_post),
        (_, PendingFlow::Condition { .. }) if !flow.is_cont() => return ok!(flow),
        _ => {}
    }
    // Materialize only when ordered reports or conclusion checks need both flows
    combine_eager(flow.into_flow(), flow_post.into_flow()).map(PendingFlow::Eager)
}

/// Runs every instruction and merges the flows; mismatches are skipped.
pub(super) fn choose_deterministic<C>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(C) -> Backtrack<PendingFlow>,
) -> Backtrack<PendingFlow> {
    // Start from an empty continuation
    let mut flow = PendingFlow::Eager(Flow::Cont(vec![]));
    for candidate in candidates {
        let flow_post = match evaluate(candidate) {
            // A mismatching instruction contributes nothing
            unmatch!(_) => continue,
            result => unwrap!(result),
        };
        // Merge, rejecting a second terminating flow
        flow = unwrap!(combine_deterministic(flow, flow_post));
    }
    ok!(flow)
}
