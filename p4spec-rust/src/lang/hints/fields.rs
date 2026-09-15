//! Field hints

use crate::lang::el::ast::Text;

/// Field labels for prose rendering
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldHint {
    fields: Vec<Text>,
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
