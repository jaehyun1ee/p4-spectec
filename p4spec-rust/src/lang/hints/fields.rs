//! Field hints
//!
//! `hint(prose_fields "a" "b")` names the fields a destructuring step binds.

use crate::lang::el::ast::Text;

/// Field labels for prose rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldHint {
    /// Field labels in order.
    fields: Vec<Text>,
}

impl FieldHint {
    /// Preserves fields without validation.
    pub fn new(fields: Vec<Text>) -> Self {
        Self { fields }
    }

    /// Borrows the labels.
    pub fn fields(&self) -> &[Text] {
        &self.fields
    }

    /// Consumes the value into fields.
    pub fn into_fields(self) -> Vec<Text> {
        self.fields
    }
}
