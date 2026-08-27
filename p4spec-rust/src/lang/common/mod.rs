//! Data shared by the language representations

use self::source::Spanned;
use crate::lang::traits::print::{Print, Printer};

pub mod ds;
pub mod notation;
pub mod noted;
pub mod source;

/// Identifier text
pub type IdKind = String;

/// Source-annotated identifier
pub type Id = Spanned<IdKind>;

impl Print for Id {
    fn print(&self, printer: &mut Printer<'_>) -> std::fmt::Result {
        printer.write(&self.node)
    }
}
