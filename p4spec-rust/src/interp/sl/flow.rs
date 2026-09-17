//! SL continuations, returns, relation results, and tail calls

use crate::{
    interp::shared::{
        backtrack::Backtrack,
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
            Backtrack::Unmatch(errors) => Backtrack::Ok(Self::Cont(errors)),
            result => result,
        }
    }

    // = Deterministic block composition

    pub(crate) fn merge(self, flow_post: Self, span: &Span) -> Backtrack<Self> {
        let flow = match (self, flow_post) {
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
                    (Flow::TailFunc(..), Flow::Result(_)) => {
                        "cannot have both tail call and result"
                    }
                    (Flow::TailFunc(..), _) => "cannot have both tail call and return",
                    (Flow::TailRel(..), Flow::Result(_)) => {
                        "cannot have both rel tail call and result"
                    }
                    (Flow::TailRel(..), _) => "cannot have both rel tail call and return",
                    (Flow::Cont(_), _) => unreachable!("continuations were combined above"),
                };
                return Backtrack::err(
                    span.clone(),
                    ErrorKind::Call(CallErrorKind::InvalidFlow { message }),
                );
            }
        };
        Backtrack::Ok(flow)
    }
}
