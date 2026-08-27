//! Syntax equality shared across language stages

use crate::yojson::ExternalData;

/// Compares syntax while ignoring source and analysis metadata
pub trait SyntaxEq<Rhs: ?Sized = Self> {
    /// Returns whether two nodes represent the same syntax
    fn syntax_eq(&self, other: &Rhs) -> bool;
}

impl SyntaxEq for ExternalData {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}
