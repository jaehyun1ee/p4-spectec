//! Printing tree notation
//!
//! Tree traversal supplies atoms and argument positions;
//! shared helpers apply spacing and silent-atom rules.

use std::fmt;

use crate::lang::traits::print::{Print, Printer};

use super::super::{print::print_piece, tree};

// = Notation forms

impl tree::Mixop {
    /// Writes atoms and arguments, separating non-empty pieces with spaces.
    ///
    /// Empty keyword atoms print nothing, not even a space;
    /// `print_arg` writes the argument at a position.
    pub fn print_with(
        &self,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        let mut is_first = true;
        let mut result = Ok(());
        self.visit(|piece| {
            if result.is_ok() {
                result = print_piece(piece, printer, &mut is_first, &mut print_arg);
            }
        });
        result
    }
}

// = Filled forms

impl Print for tree::Mixop {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.print_with(printer, |_, printer| printer.write("%"))
    }
}

impl<T> tree::Mixfix<T> {
    /// Writes atoms and arguments, separating non-empty pieces with spaces.
    pub fn print_with(
        &self,
        printer: &mut Printer<'_>,
        mut print_arg: impl FnMut(&T, &mut Printer<'_>) -> fmt::Result,
    ) -> fmt::Result {
        self.mixop
            .print_with(printer, |pos, printer| print_arg(&self.args[pos], printer))
    }
}
