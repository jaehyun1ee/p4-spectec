//! Field hints

use crate::lang::el::ast::{Exp, ExpKind, Text};
use thiserror::Error;

/// Field labels for prose rendering
///
/// `new` does not validate field count;
/// call `validate` when the target arity is known
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

/// Converts to string
pub fn to_string(hint: &FieldHint) -> String {
    format!(
        "hint(fields {})",
        hint.fields
            .iter()
            .map(|text| crate::lang::el::print::string_of_text(text))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

// Creating hints

/// Initializes the value
pub fn init(hint_exp: &Exp) -> Option<FieldHint> {
    let fields = match &hint_exp.node {
        ExpKind::Text(text) => Some(vec![text.clone()]),
        ExpKind::Seq(hint_exps) => hint_exps
            .iter()
            .map(|hint_exp| match &hint_exp.node {
                ExpKind::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }?;
    Some(FieldHint::new(fields))
}

// Validating hints

/// Validates that field count matches `arity`
pub fn validate(hint: &FieldHint, arity: usize) -> Result<(), FieldError> {
    if hint.fields.len() == arity {
        Ok(())
    } else {
        Err(FieldError::ArityMismatch {
            expected: arity,
            actual: hint.fields.len(),
        })
    }
}
