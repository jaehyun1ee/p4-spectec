//! SL continuations, returns, relation results, and tail calls

use crate::{
    interp::shared::{
        backtrack::{Backtrack, ok, unwrap},
        error::{CallErrorKind, Error, ErrorKind, PremErrorKind},
    },
    lang::{common::source::Span, data::value::Value, sl::ast},
};

#[derive(Clone, Debug)]
pub enum Flow {
    Cont(Vec<Error>),
    Return(Value),
    Result(Vec<Value>),
    TailFunc(ast::Id, Vec<ast::Typ>, Vec<Value>),
    TailRel(ast::Id, Vec<Value>),
}

impl Flow {
    // = Continuation

    pub(crate) fn cont(span: Span, error: PremErrorKind) -> Self {
        Self::Cont(vec![Error::new(ErrorKind::Prem(error), span)])
    }

    pub(crate) fn cont_from_unmatch(result: Backtrack<Self>) -> Backtrack<Self> {
        match result {
            Backtrack::Unmatch(errors) => ok!(Self::Cont(errors)),
            result => result,
        }
    }
}

// = Sequential choice

fn retain_deepest_errors(errors: &mut Vec<Error>, errors_post: Vec<Error>) {
    if errors_post.iter().map(Error::depth).max().unwrap_or(0)
        >= errors.iter().map(Error::depth).max().unwrap_or(0)
    {
        *errors = errors_post;
    }
}

pub(crate) fn choose_sequential<C>(
    mut candidates: impl DoubleEndedIterator<Item = C>,
    mut evaluate: impl FnMut(C, bool) -> Backtrack<Flow>,
) -> Backtrack<Flow> {
    let Some(candidate_last) = candidates.next_back() else {
        return ok!(Flow::Cont(vec![]));
    };
    let mut errors = Vec::new();
    for candidate in candidates {
        match unwrap!(evaluate(candidate, false)) {
            Flow::Cont(errors_post) => retain_deepest_errors(&mut errors, errors_post),
            flow => return ok!(flow),
        }
    }
    match unwrap!(evaluate(candidate_last, true)) {
        Flow::Cont(errors_post) => {
            retain_deepest_errors(&mut errors, errors_post);
            ok!(Flow::Cont(errors))
        }
        flow => ok!(flow),
    }
}

// = Deterministic choice

fn combine_deterministic(flow: Flow, flow_post: Flow, span: &Span) -> Backtrack<Flow> {
    let flow = match (flow, flow_post) {
        (Flow::Cont(mut errors), Flow::Cont(errors_post)) => {
            errors.extend(errors_post);
            Flow::Cont(errors)
        }
        (Flow::Cont(_), flow) | (flow, Flow::Cont(_)) => flow,
        (Flow::Return(_), Flow::Return(_))
        | (Flow::Result(_), Flow::Result(_))
        | (Flow::TailFunc(..) | Flow::TailRel(..), Flow::TailFunc(..) | Flow::TailRel(..)) => {
            return Backtrack::err(
                span.clone(),
                ErrorKind::Call(CallErrorKind::InstructionNondeterminism),
            );
        }
        (flow_pre, flow_post) => {
            let message = match (flow_pre, flow_post) {
                (Flow::Result(_), Flow::Return(_)) => "cannot have both result and return",
                (Flow::Result(_), _) => "cannot have both result and tail call",
                (Flow::Return(_), Flow::Result(_)) => "cannot have both return and result",
                (Flow::Return(_), _) => "cannot have both return and tail call",
                (Flow::TailFunc(..), Flow::Result(_)) => "cannot have both tail call and result",
                (Flow::TailFunc(..), _) => "cannot have both tail call and return",
                (Flow::TailRel(..), Flow::Result(_)) => "cannot have both rel tail call and result",
                (Flow::TailRel(..), _) => "cannot have both rel tail call and return",
                (Flow::Cont(_), _) => unreachable!("continuations were combined above"),
            };
            return Backtrack::err(
                span.clone(),
                ErrorKind::Call(CallErrorKind::InvalidFlow { message }),
            );
        }
    };
    ok!(flow)
}

pub(crate) fn choose_deterministic<C>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(C) -> Backtrack<Flow>,
    mut span_of: impl FnMut(&C) -> Span,
) -> Backtrack<Flow> {
    let mut flow = Flow::Cont(vec![]);
    for candidate in candidates {
        let span = span_of(&candidate);
        let flow_post = match evaluate(candidate) {
            Backtrack::Unmatch(_) => continue,
            result => unwrap!(result),
        };
        flow = unwrap!(combine_deterministic(flow, flow_post, &span));
    }
    ok!(flow)
}
