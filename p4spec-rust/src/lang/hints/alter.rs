//! Alteration hints for prose rendering

use crate::lang::{
    el::{
        ast::{Atom, Exp, ExpKind, Hole as ElHole, Text},
        print,
    },
    hints::input,
};
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

    #[error("alteration hint index {0} is missing from the realignment")]
    MissingIndex(i64),
}

/// Converts to string
pub fn to_string(hint: &AlterationHint) -> String {
    format!("hint(alter {})", string(hint))
}
fn string(hint: &AlterationHint) -> String {
    match hint {
        AlterationHint::Text(text) => print::string_of_text(text),
        AlterationHint::Atom(atom) => print::string_of_atom(atom),
        AlterationHint::Seq(hints) => hints.iter().map(string).collect::<Vec<_>>().join(" "),
        AlterationHint::Brack(atom_l, hint, atom_r) => format!(
            "{} {} {}",
            print::string_of_atom(atom_l),
            string(hint),
            print::string_of_atom(atom_r)
        ),
        AlterationHint::Hole(Hole::Next) => "%".into(),
        AlterationHint::Hole(Hole::Num(index)) => format!("%{index}"),
        AlterationHint::Fuse(hint_l, hint_r) => format!("{}#{}", string(hint_l), string(hint_r)),
        AlterationHint::Other(exp) => print::string_of_exp(exp),
    }
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
// Validating hints

/// Validates every hole against `items`
pub fn validate<Item>(hint: &AlterationHint, items: &[Item]) -> Result<(), AlterationError> {
    validate_at(hint, items, 0).map(|_| ())
}
fn validate_at<Item>(
    hint: &AlterationHint,
    items: &[Item],
    cursor: usize,
) -> Result<usize, AlterationError> {
    match hint {
        AlterationHint::Text(_) | AlterationHint::Atom(_) | AlterationHint::Other(_) => Ok(cursor),
        AlterationHint::Seq(hints) => hints
            .iter()
            .try_fold(cursor, |cursor, hint| validate_at(hint, items, cursor)),
        AlterationHint::Brack(_, hint, _) => validate_at(hint, items, cursor),
        AlterationHint::Hole(Hole::Next) if cursor < items.len() => Ok(cursor + 1),
        AlterationHint::Hole(Hole::Next) => Err(AlterationError::IndexOutOfBounds {
            index: i64::try_from(cursor).unwrap_or(i64::MAX),
            item_count: items.len(),
        }),
        AlterationHint::Hole(Hole::Num(index))
            if *index >= 0 && (*index as usize) < items.len() =>
        {
            Ok(cursor)
        }
        AlterationHint::Hole(Hole::Num(index)) => Err(AlterationError::IndexOutOfBounds {
            index: *index,
            item_count: items.len(),
        }),
        AlterationHint::Fuse(hint_l, hint_r) => {
            validate_at(hint_r, items, validate_at(hint_l, items, cursor)?)
        }
    }
}
// Re-alignment of alternation indices

/// Applies collect
pub fn collect(hint: &AlterationHint) -> Vec<i64> {
    fn collect_inner(hint: &AlterationHint, indices: &mut Vec<i64>) {
        match hint {
            AlterationHint::Hole(Hole::Num(index)) => indices.insert(0, *index),
            AlterationHint::Seq(hints) => {
                for hint in hints {
                    collect_inner(hint, indices)
                }
            }
            AlterationHint::Brack(_, hint, _) => collect_inner(hint, indices),
            AlterationHint::Fuse(hint_l, hint_r) => {
                collect_inner(hint_l, indices);
                collect_inner(hint_r, indices)
            }
            _ => {}
        }
    }
    let mut indices = Vec::new();
    collect_inner(hint, &mut indices);
    indices
}
/// Renumbers output holes after relation input positions
///
/// Returns an error when a referenced output position is absent
pub fn realign(
    hint: &AlterationHint,
    inputs: &input::InputHint,
) -> Result<AlterationHint, AlterationError> {
    let outputs = collect(hint);
    let mut all = inputs.indices().to_vec();
    all.extend(&outputs);
    all.sort();
    let mut pairs = Vec::new();
    for index in all {
        if outputs.contains(&index) {
            pairs.push((index, pairs.len() as i64));
        }
    }
    fn realign_inner(
        hint: &AlterationHint,
        index_pairs: &[(i64, i64)],
    ) -> Result<AlterationHint, AlterationError> {
        Ok(match hint {
            AlterationHint::Seq(hints) => AlterationHint::Seq(
                hints
                    .iter()
                    .map(|hint| realign_inner(hint, index_pairs))
                    .collect::<Result<_, _>>()?,
            ),
            AlterationHint::Brack(atom_l, hint, atom_r) => AlterationHint::Brack(
                atom_l.clone(),
                Box::new(realign_inner(hint, index_pairs)?),
                atom_r.clone(),
            ),
            AlterationHint::Hole(Hole::Num(index)) => AlterationHint::Hole(Hole::Num(
                index_pairs
                    .iter()
                    .find(|(index_old, _)| index_old == index)
                    .ok_or(AlterationError::MissingIndex(*index))?
                    .1,
            )),
            AlterationHint::Fuse(hint_l, hint_r) => AlterationHint::Fuse(
                Box::new(realign_inner(hint_l, index_pairs)?),
                Box::new(realign_inner(hint_r, index_pairs)?),
            ),
            _ => hint.clone(),
        })
    }
    realign_inner(hint, &pairs)
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

/// Renders a validated alteration hint
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
