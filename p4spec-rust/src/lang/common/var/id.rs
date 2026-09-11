//! Identifiers shared by the language representations

use crate::lang::traits::print::{Print, Printer};

use crate::lang::common::source::Phrase;

/// Source-annotated identifier
pub type Id = Phrase<String>;

impl Print for Id {
    fn print(&self, printer: &mut Printer<'_>) -> std::fmt::Result {
        printer.write(&self.node)
    }
}
