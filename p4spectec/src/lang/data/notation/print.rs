//! Rendering notation atoms and arguments in reading order
//!
//! Tree and flat visits supply the pieces; empty keyword atoms are silent.
//! `tree_with` and `flat_with` separate the remaining pieces with spaces
//! and delegate argument rendering to the caller.

use std::fmt;

use crate::lang::{
    common::notation::atom::Atom,
    traits::print::{Print, Printer},
};

use super::{MixopArena, Piece, flat, tree};

/// Writes atoms and arguments, separating non-empty pieces with spaces.
///
/// Empty keyword atoms print nothing, not even a space;
/// `print_arg` writes the argument at a position.
pub(super) fn tree_with(
    mixop: &tree::Mixop,
    printer: &mut Printer<'_>,
    mut print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    let mut is_first = true;
    let mut result = Ok(());
    mixop.visit(|piece| {
        if result.is_ok() {
            result = print_piece(piece, printer, &mut is_first, &mut print_arg);
        }
    });
    result
}

/// Writes atoms and arguments, separating non-empty pieces with spaces.
///
/// Empty keyword atoms print nothing, not even a space;
/// `print_arg` writes the argument at a position.
pub(crate) fn flat_with(
    arena_mixop: &MixopArena,
    mixop: flat::Mixop,
    printer: &mut Printer<'_>,
    mut print_arg: impl FnMut(usize, &mut Printer<'_>) -> fmt::Result,
) -> fmt::Result {
    let mut is_first = true;
    let mut result = Ok(());
    flat::visit(arena_mixop, mixop, |piece| {
        if result.is_ok() {
            result = print_piece(piece, printer, &mut is_first, &mut print_arg);
        }
    });
    result
}

/// Prints one piece using the same spacing rules for both representations.
fn print_piece(
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
