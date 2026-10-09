//! Rendering notation atoms and arguments in reading order
//!
//! Tree and flat visits supply the pieces; empty keyword atoms are silent.
//! `print_piece` separates the remaining pieces with spaces
//! and delegates argument rendering to the caller.

use std::fmt;

use crate::lang::{
    common::notation::atom::Atom,
    traits::print::{Print, Printer},
};

use super::Piece;

// = Pieces

/// Prints one piece using the same spacing rules for both representations.
pub(super) fn print_piece(
    piece: Piece<'_>,
    printer: &mut Printer<'_>,
    is_first: &mut bool,
    print_arg: &mut impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    match piece {
        Piece::Atom(atom) if matches!(&atom.node, Atom::Keyword(keyword) if keyword.is_empty()) => {
            Ok(())
        }
        Piece::Atom(atom) => print_sep(printer, is_first).and_then(|()| atom.print(printer)),
        Piece::Arg(pos) => print_sep(printer, is_first).and_then(|()| print_arg(pos, printer)),
    }
}

/// Writes a space before every piece but the first.
fn print_sep(printer: &mut Printer<'_>, is_first: &mut bool) -> fmt::Result {
    if *is_first {
        *is_first = false;
        Ok(())
    } else {
        printer.write(" ")
    }
}
