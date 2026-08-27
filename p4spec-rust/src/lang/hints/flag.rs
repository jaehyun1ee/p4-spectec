//! Flag hints

use crate::lang::el::ast::Hint;

pub type Flag = bool;

/// Converts to string
pub fn to_string(hint: Flag) -> String {
    if hint {
        "hint(flag)".into()
    } else {
        String::new()
    }
}

// Creating hints

/// Initializes the value
pub fn init(hints: &[Hint], hint_id: &str) -> Flag {
    hints.iter().any(|hint| hint.0.node == hint_id)
}
