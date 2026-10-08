//! Source locations of tree notation
//!
//! Covers the atoms and arguments of a filled form in notation order.

use crate::lang::{common::source::Span, traits::at::At};

use super::super::Piece;
use super::Mixfix;

impl<T: At> At for Mixfix<T> {
    fn at(&self) -> Span {
        // Cover atoms and arguments, so empty sequences add no default span
        let mut spans = Vec::new();
        self.mixop.visit(|piece| match piece {
            Piece::Atom(atom) => spans.push(atom.at()),
            Piece::Arg(pos) => spans.push(self.args[pos].at()),
        });
        spans.at()
    }
}
