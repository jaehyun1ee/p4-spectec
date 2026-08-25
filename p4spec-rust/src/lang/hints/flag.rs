//! Flag hints

use crate::lang::el::ast::Hint;

pub type Flag = bool;

pub fn to_string(hint: Flag) -> String {
    if hint {
        "hint(flag)".into()
    } else {
        String::new()
    }
}

// Creating hints

pub fn init(hints: &[Hint], hint_id: &str) -> Flag {
    hints.iter().any(|hint| hint.hintid.node == hint_id)
}
