//! PL block continuations, function returns, and relation results
//!
//! PL concludes directly; invocations evaluate their callable bodies.
//! Alternative selection combines conclusions and retains failure diagnostics.

use crate::{
    interp::shared::{
        backtrack::{Backtrack, err, ok},
        error::{CallErrorKind, Error, ErrorKind, PremErrorKind},
    },
    lang::{common::source::Span, data::value::Value},
};

/// Records whether an instruction continues or concludes its callable.
#[derive(Clone, Debug)]
pub enum Flow {
    /// Fell through, with the failures met so far.
    Cont(Vec<Error>),
    /// A function body returned a value.
    Return(Value),
    /// A relation body produced its outputs.
    Result(Vec<Value>),
}

impl Flow {
    // = Continuation

    /// Creates a recoverable continuation with its premise diagnostic.
    pub(crate) fn cont(span: Span, error: PremErrorKind) -> Self {
        Self::Cont(vec![Error::new(ErrorKind::Prem(error), span)])
    }
}

// = Sequential choice

/// Keeps the most deeply nested failure, preferring the later one on ties.
pub(crate) fn retain_deepest_errors(errors: &mut Vec<Error>, errors_post: Vec<Error>) {
    if errors_post.iter().map(Error::depth).max().unwrap_or(0)
        >= errors.iter().map(Error::depth).max().unwrap_or(0)
    {
        *errors = errors_post;
    }
}

// = Deterministic choice

/// Merges alternative outcomes and rejects multiple conclusions.
pub(crate) fn combine_deterministic(flow: Flow, flow_post: Flow, span: &Span) -> Backtrack<Flow> {
    let flow = match (flow, flow_post) {
        // Both continue: merge the failures
        (Flow::Cont(mut errors), Flow::Cont(errors_post)) => {
            errors.extend(errors_post);
            Flow::Cont(errors)
        }
        // One concluded: keep it
        (Flow::Cont(_), flow) | (flow, Flow::Cont(_)) => flow,
        // Two of the same kind: nondeterminism
        (Flow::Return(_), Flow::Return(_)) | (Flow::Result(_), Flow::Result(_)) => {
            return err!(span.clone(), ErrorKind::Call(CallErrorKind::InstructionNondeterminism));
        }
        // Different conclusion kinds cannot belong to the same callable
        _ => {
            return err!(
                span.clone(),
                ErrorKind::Call(CallErrorKind::InvalidFlow {
                    message: "incompatible PL conclusions"
                })
            );
        }
    };
    ok!(flow)
}
