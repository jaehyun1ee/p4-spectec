//! Alteration hints for prose rendering
//!
//! `hint(prose "the type of" % "is" %)` is a template:
//! text and atoms print as they are, `%` holes take the items being described.
//! `alternate` renders one through a `Renderer` for the output format.

use crate::lang::el::ast::{Atom, Exp, ExpKind, Hole as ElHole, Text};
use thiserror::Error;

// Alternation hints

/// A positional hole in an alteration hint.
#[derive(Clone, Debug, PartialEq)]
pub enum Hole {
    /// `%`, the next item in cursor order.
    Next,
    /// `%N`, the N-th item.
    Num(usize),
}

/// A prose rendering template
///
/// `Hole::Next` consumes items in cursor order;
/// `Hole::Num` selects an explicit item index.
#[derive(Clone, Debug, PartialEq)]
pub enum AlterationHint {
    /// Literal text.
    Text(Text),
    /// A notation atom.
    Atom(Atom),
    /// Pieces in order.
    Seq(Vec<AlterationHint>),
    /// A piece between bracket atoms.
    Brack(Atom, Box<AlterationHint>, Atom),
    /// An item placeholder.
    Hole(Hole),
    /// Two pieces joined without a separator.
    Fuse(Box<AlterationHint>, Box<AlterationHint>),
    /// Any other expression, rendered by the caller.
    Other(Exp),
}

/// A failure rendering an alteration hint.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AlterationError {
    /// A hole asked for an item that does not exist.
    #[error("alteration hint index {index} is out of bounds for {item_count} items")]
    IndexOutOfBounds { index: usize, item_count: usize },
}

// Creating hints

/// Reads a template from a hint expression; unknown forms become `Other`.
pub fn init(exp: &Exp) -> Option<AlterationHint> {
    Some(match &exp.node {
        // Text, atoms, sequences, brackets, holes, and fuses map directly
        ExpKind::Text(text) => AlterationHint::Text(text.clone()),
        ExpKind::Atom(atom) => AlterationHint::Atom(atom.clone()),
        ExpKind::Seq(exps) => AlterationHint::Seq(exps.iter().map(init).collect::<Option<_>>()?),
        ExpKind::Brack(atom_l, exp, atom_r) => {
            AlterationHint::Brack(atom_l.clone(), Box::new(init(exp)?), atom_r.clone())
        }
        // `%` and `%N`; `%%` and `!%` have no template meaning
        ExpKind::Hole(ElHole::Next) => AlterationHint::Hole(Hole::Next),
        ExpKind::Hole(ElHole::Num(index)) => AlterationHint::Hole(Hole::Num(*index)),
        ExpKind::Fuse(exp_l, exp_r) => {
            AlterationHint::Fuse(Box::new(init(exp_l)?), Box::new(init(exp_r)?))
        }
        // Anything else is kept as an expression for the renderer
        _ => AlterationHint::Other(exp.clone()),
    })
}

// Alternation

/// Renders alteration pieces into a caller-defined output.
pub trait Renderer<Item> {
    /// The rendered form.
    type Output: Clone;
    /// Nothing.
    fn empty(&self) -> Self::Output;
    /// Literal text, or nothing if the text renders empty.
    fn text(&self, text: &str) -> Option<Self::Output>;
    /// A notation atom.
    fn atom(&self, atom: &Atom) -> Self::Output;
    /// Pieces in sequence.
    fn join(&self, items: Vec<Self::Output>) -> Self::Output;
    /// Two pieces without a separator.
    fn fuse(&self, output_l: Self::Output, output_r: Self::Output) -> Self::Output;
    /// An expression the template did not understand.
    fn other(&self, exp: &Exp) -> Self::Output;
    /// One of the items being described.
    fn item(&self, item: &Item) -> Self::Output;
}

/// Renders an alteration hint
///
/// Returns an error when a hole cannot select an item.
pub fn alternate<Item, R: Renderer<Item>>(
    hint: &AlterationHint,
    items: &[Item],
    renderer: &R,
) -> Result<R::Output, AlterationError> {
    /// Renders a piece, returning the advanced cursor and the output if any.
    fn go<Item, R: Renderer<Item>>(
        hint: &AlterationHint,
        items: &[Item],
        cursor: usize,
        renderer: &R,
    ) -> Result<(usize, Option<R::Output>), AlterationError> {
        Ok(match hint {
            AlterationHint::Text(text) => (cursor, renderer.text(text)),
            AlterationHint::Atom(atom) => (cursor, Some(renderer.atom(atom))),
            // Pieces share one cursor, so `%` holes take successive items
            AlterationHint::Seq(hints) => {
                let mut cursor_next = cursor;
                let mut outputs = Vec::new();
                for hint in hints {
                    let (cursor_after, output) = go(hint, items, cursor_next, renderer)?;
                    cursor_next = cursor_after;
                    outputs.push(output.unwrap_or_else(|| renderer.empty()));
                }
                (cursor_next, Some(renderer.join(outputs)))
            }
            // Brackets surround the inner piece, dropped if it rendered nothing
            AlterationHint::Brack(atom_l, hint, atom_r) => {
                let (cursor_next, output) = go(hint, items, cursor, renderer)?;
                let mut outputs = vec![renderer.atom(atom_l)];
                if let Some(output) = output {
                    outputs.push(output);
                }
                outputs.push(renderer.atom(atom_r));
                (cursor_next, Some(renderer.join(outputs)))
            }
            // The next item, advancing the cursor
            AlterationHint::Hole(Hole::Next) => {
                let item = items.get(cursor).ok_or(AlterationError::IndexOutOfBounds {
                    index: cursor,
                    item_count: items.len(),
                })?;
                (cursor + 1, Some(renderer.item(item)))
            }
            // A specific item, leaving the cursor alone
            AlterationHint::Hole(Hole::Num(index)) => {
                let item = items.get(*index).ok_or(AlterationError::IndexOutOfBounds {
                    index: *index,
                    item_count: items.len(),
                })?;
                (cursor, Some(renderer.item(item)))
            }
            // Both sides render, the right one continuing the left's cursor
            AlterationHint::Fuse(hint_l, hint_r) => {
                let (cursor_mid, output_l) = go(hint_l, items, cursor, renderer)?;
                let (cursor_next, output_r) = go(hint_r, items, cursor_mid, renderer)?;
                (
                    cursor_next,
                    Some(renderer.fuse(
                        output_l.unwrap_or_else(|| renderer.empty()),
                        output_r.unwrap_or_else(|| renderer.empty()),
                    )),
                )
            }
            AlterationHint::Other(exp) => (cursor, Some(renderer.other(exp))),
        })
    }
    Ok(go(hint, items, 0, renderer)?
        .1
        .unwrap_or_else(|| renderer.empty()))
}
