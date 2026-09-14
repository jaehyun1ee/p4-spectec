//! Alteration hints for prose rendering

use crate::lang::el::ast::{Atom, Exp, ExpKind, Hole as ElHole, Text};
use thiserror::Error;

// Alternation hints

/// A positional hole in an alteration hint
#[derive(Clone, Debug, PartialEq)]
pub enum Hole {
    Next,
    Num(i64),
}

/// A prose rendering template
///
/// `Hole::Next` consumes items in cursor order;
/// `Hole::Num` selects an explicit item index
#[derive(Clone, Debug, PartialEq)]
pub enum AlterationHint {
    Text(Text),
    Atom(Atom),
    Seq(Vec<AlterationHint>),
    Brack(Atom, Box<AlterationHint>, Atom),
    Hole(Hole),
    Fuse(Box<AlterationHint>, Box<AlterationHint>),
    Other(Exp),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AlterationError {
    #[error("alteration hint index {index} is out of bounds for {item_count} items")]
    IndexOutOfBounds { index: i64, item_count: usize },
}

// Creating hints

/// Initializes the value
pub fn init(exp: &Exp) -> Option<AlterationHint> {
    Some(match &exp.node {
        ExpKind::Text(text) => AlterationHint::Text(text.clone()),
        ExpKind::Atom(atom) => AlterationHint::Atom(atom.clone()),
        ExpKind::Seq(exps) => AlterationHint::Seq(exps.iter().map(init).collect::<Option<_>>()?),
        ExpKind::Brack(atom_l, exp, atom_r) => {
            AlterationHint::Brack(atom_l.clone(), Box::new(init(exp)?), atom_r.clone())
        }
        ExpKind::Hole(ElHole::Next) => AlterationHint::Hole(Hole::Next),
        ExpKind::Hole(ElHole::Num(index)) => AlterationHint::Hole(Hole::Num(*index)),
        ExpKind::Fuse(exp_l, exp_r) => {
            AlterationHint::Fuse(Box::new(init(exp_l)?), Box::new(init(exp_r)?))
        }
        _ => AlterationHint::Other(exp.clone()),
    })
}

// Alternation

/// Renders alteration pieces into a caller-defined output
pub trait Renderer<Item> {
    type Output: Clone;
    fn empty(&self) -> Self::Output;
    fn text(&self, text: &str) -> Option<Self::Output>;
    fn atom(&self, atom: &Atom) -> Self::Output;
    fn join(&self, items: Vec<Self::Output>) -> Self::Output;
    fn fuse(&self, output_l: Self::Output, output_r: Self::Output) -> Self::Output;
    fn other(&self, exp: &Exp) -> Self::Output;
    fn item(&self, item: &Item) -> Self::Output;
}

/// Renders an alteration hint
///
/// Returns an error when a hole cannot select an item
pub fn alternate<Item, R: Renderer<Item>>(
    hint: &AlterationHint,
    items: &[Item],
    renderer: &R,
) -> Result<R::Output, AlterationError> {
    fn go<Item, R: Renderer<Item>>(
        hint: &AlterationHint,
        items: &[Item],
        cursor: usize,
        renderer: &R,
    ) -> Result<(usize, Option<R::Output>), AlterationError> {
        Ok(match hint {
            AlterationHint::Text(text) => (cursor, renderer.text(text)),
            AlterationHint::Atom(atom) => (cursor, Some(renderer.atom(atom))),
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
            AlterationHint::Brack(atom_l, hint, atom_r) => {
                let (cursor_next, output) = go(hint, items, cursor, renderer)?;
                let mut outputs = vec![renderer.atom(atom_l)];
                if let Some(output) = output {
                    outputs.push(output);
                }
                outputs.push(renderer.atom(atom_r));
                (cursor_next, Some(renderer.join(outputs)))
            }
            AlterationHint::Hole(Hole::Next) => {
                let item = items.get(cursor).ok_or(AlterationError::IndexOutOfBounds {
                    index: i64::try_from(cursor).unwrap_or(i64::MAX),
                    item_count: items.len(),
                })?;
                (cursor + 1, Some(renderer.item(item)))
            }
            AlterationHint::Hole(Hole::Num(index)) => {
                let item = usize::try_from(*index)
                    .ok()
                    .and_then(|index| items.get(index))
                    .ok_or(AlterationError::IndexOutOfBounds {
                        index: *index,
                        item_count: items.len(),
                    })?;
                (cursor, Some(renderer.item(item)))
            }
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
