//! Flag hints

use crate::lang::el::ast::Hint;

pub type T = bool;

pub fn init(hints: &[Hint], hint_id: &str) -> T {
    hints.iter().any(|hint| hint.hintid.node == hint_id)
}
