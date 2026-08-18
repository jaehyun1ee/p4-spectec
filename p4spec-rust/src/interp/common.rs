use thiserror::Error;

use crate::domain::source::Region;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{message} at {span}")]
pub struct InterpError {
    pub span: Region,
    pub message: String,
}

impl InterpError {
    pub fn new(span: Region, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }
}
