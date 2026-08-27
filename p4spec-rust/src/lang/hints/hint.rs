//! Generic expression hints

pub type Hint = crate::lang::el::ast::Exp;

/// Converts to string
pub fn to_string(hint: &Hint) -> String {
    crate::lang::traits::print::Print::to_string(hint)
}
