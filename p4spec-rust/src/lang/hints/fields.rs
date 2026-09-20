//! Field hints

use crate::lang::el::ast::{Exp, ExpKind, Text};
use thiserror::Error;

// == Field hints

/// Field labels for prose rendering
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldHint {
    fields: Vec<Text>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FieldError {
    #[error("field hint expects {expected} strings, but got {actual}")]
    ArityMismatch { expected: usize, actual: usize },
}

impl FieldHint {
    /// Preserves fields without validation
    pub fn new(fields: Vec<Text>) -> Self {
        Self { fields }
    }

    /// Applies fields
    pub fn fields(&self) -> &[Text] {
        &self.fields
    }

    /// Consumes the value into fields
    pub fn into_fields(self) -> Vec<Text> {
        self.fields
    }
}

// == Initialization

/// Initializes a field hint from one text or a sequence of texts
pub fn init(exp: &Exp) -> Option<FieldHint> {
    let fields = match &exp.node {
        ExpKind::Text(text) => vec![text.clone()],
        ExpKind::Seq(exps) => exps
            .iter()
            .map(|exp| match &exp.node {
                ExpKind::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect::<Option<_>>()?,
        _ => return None,
    };
    Some(FieldHint::new(fields))
}

// == Validation

/// Validates that the field count matches `arity`
pub fn validate(hint: &FieldHint, arity: usize) -> Result<(), FieldError> {
    if hint.fields.len() == arity {
        Ok(())
    } else {
        Err(FieldError::ArityMismatch { expected: arity, actual: hint.fields.len() })
    }
}
