use thiserror::Error;

use crate::domain::source::Region;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message} at {span}")]
pub struct InterpError {
    pub span: Region,
    pub message: String,
    unmatched: bool,
}

impl InterpError {
    pub fn new(span: Region, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            unmatched: false,
        }
    }

    pub fn unmatch(span: Region, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            unmatched: true,
        }
    }

    pub fn is_unmatch(&self) -> bool {
        self.unmatched
    }
}
