//! PL block continuations, returns, results, and tail calls

use crate::{
    interp::shared::{
        backtrack::{Backtrack, err, ok},
        error::{CallErrorKind, Error, ErrorKind, PremErrorKind},
    },
    lang::{common::source::Span, data::value::Value, pl::ast},
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
    pub(crate) fn cont(span: Span, error: PremErrorKind) -> Self {
        Self::Cont(vec![Error::new(ErrorKind::Prem(error), span)])
    }
}

pub(crate) fn retain_deepest_errors(errors: &mut Vec<Error>, errors_post: Vec<Error>) {
    if errors_post.iter().map(Error::depth).max().unwrap_or(0)
        >= errors.iter().map(Error::depth).max().unwrap_or(0)
    {
        *errors = errors_post;
    }
}

pub(crate) fn combine_deterministic(flow: Flow, flow_post: Flow, span: &Span) -> Backtrack<Flow> {
    let flow = match (flow, flow_post) {
        (Flow::Cont(mut errors), Flow::Cont(errors_post)) => {
            errors.extend(errors_post);
            Flow::Cont(errors)
        }
        (Flow::Cont(_), flow) | (flow, Flow::Cont(_)) => flow,
        (Flow::Return(_), Flow::Return(_))
        | (Flow::Result(_), Flow::Result(_))
        | (Flow::TailFunc(..) | Flow::TailRel(..), Flow::TailFunc(..) | Flow::TailRel(..)) => {
            return err!(span.clone(), ErrorKind::Call(CallErrorKind::InstructionNondeterminism));
        }
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
