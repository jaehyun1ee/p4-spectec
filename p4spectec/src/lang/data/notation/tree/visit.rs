//! Traversal of tree notation
//!
//! `visit` supplies atoms and numbered argument positions in reading order.

use super::super::Piece;
use super::Mixop;

// = Traversal

impl Mixop {
    /// Visits atoms and argument positions in reading order.
    pub(super) fn visit<'a>(&'a self, mut visit_piece: impl FnMut(Piece<'a>)) {
        let mut pos = 0;
        self.visit_inner(&mut pos, &mut visit_piece);
    }

    /// Visits one mixop, threading the position.
    fn visit_inner<'a>(&'a self, pos: &mut usize, visit_piece: &mut impl FnMut(Piece<'a>)) {
        match self {
            Mixop::Arg => {
                visit_piece(Piece::Arg(*pos));
                *pos += 1;
            }
            Mixop::Atom(atom) => visit_piece(Piece::Atom(atom)),
            Mixop::Brack(atom_l, mixop, atom_r) => {
                visit_piece(Piece::Atom(atom_l));
                mixop.visit_inner(pos, visit_piece);
                visit_piece(Piece::Atom(atom_r));
            }
            Mixop::Infix(mixop_l, atom, mixop_r) => {
                mixop_l.visit_inner(pos, visit_piece);
                visit_piece(Piece::Atom(atom));
                mixop_r.visit_inner(pos, visit_piece);
            }
            Mixop::Seq(mixops) => {
                for mixop in mixops {
                    mixop.visit_inner(pos, visit_piece);
                }
            }
        }
    }
}
