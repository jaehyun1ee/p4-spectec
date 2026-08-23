//! Flag hints

use crate::lang::el::ast::Hint;

pub type T = bool;

pub fn to_string(hint: T) -> String {
    if hint {
        "hint(flag)".into()
    } else {
        String::new()
    }
}

pub fn init(hints: &[Hint], hint_id: &str) -> T {
    hints.iter().any(|hint| hint.hintid.node == hint_id)
}
