//! Input hints for relations
//!
//! `hint(input %0 %2)` says which notation positions of a relation are inputs;
//! the rest are outputs the relation computes.
//! `split` and `combine` move between source order and the input/output lists.

use crate::lang::{
    el::ast::{Exp, ExpKind, Hole},
    traits::eq::SyntaxEq,
};
use thiserror::Error;

/// Relation input positions in source order
///
/// `new` does not validate indices;
/// call `validate` when the relation arity is known;
/// operations such as `split` validate before consuming items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputHint {
    /// Input positions, in the order the hint lists them.
    indices: Vec<usize>,
}

/// An invalid hint or a list that does not fit it.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InputError {
    /// No input position at all.
    #[error("input hint is empty")]
    Empty,

    /// A position listed twice.
    #[error("input hint contains duplicate index {0}")]
    DuplicateIndex(usize),

    /// A position past the relation's arity.
    #[error("input hint index {index} is out of bounds for arity {arity}")]
    IndexOutOfBounds { index: usize, arity: usize },

    /// The input list has the wrong length.
    #[error("input hint expects {expected} input items, but got {actual}")]
    InputCountMismatch { expected: usize, actual: usize },

    /// The output list has the wrong length.
    #[error("input hint expects {expected} output items, but got {actual}")]
    OutputCountMismatch { expected: usize, actual: usize },
}

impl InputHint {
    /// Preserves indices without validation.
    pub fn new(indices: Vec<usize>) -> Self {
        Self { indices }
    }

    /// Borrows positions in source order.
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    /// Returns positions in source order.
    pub fn into_indices(self) -> Vec<usize> {
        self.indices
    }
}

// Syntax equivalence of hints

impl SyntaxEq for InputHint {
    fn syntax_eq(&self, other: &Self) -> bool {
        self == other
    }
}

// Creating hints

/// Reads a hint from `%N` holes, one or a sequence; anything else is no hint.
pub fn init(hint_exp: &Exp) -> Option<InputHint> {
    let indices = match &hint_exp.node {
        // A sequence of `%N` holes, all of which must be holes
        ExpKind::Seq(hint_exps) => hint_exps
            .iter()
            .map(|hint_exp| match hint_exp.node {
                ExpKind::Hole(Hole::Num(index)) => Some(index),
                _ => None,
            })
            .collect(),
        // A single hole
        ExpKind::Hole(Hole::Num(index)) => Some(vec![*index]),
        // Anything else is not an input hint
        _ => None,
    }?;
    Some(InputHint::new(indices))
}

// Validating hints

/// Validates non-empty, unique positions within `arity`
pub fn validate(hint: &InputHint, arity: usize) -> Result<(), InputError> {
    if hint.indices.is_empty() {
        return Err(InputError::Empty);
    }
    // Each position at most once
    for (position, index) in hint.indices.iter().enumerate() {
        if hint.indices[..position].contains(index) {
            return Err(InputError::DuplicateIndex(*index));
        }
    }
    // Every position within the arity
    if let Some(index) = hint.indices.iter().find(|index| **index >= arity) {
        return Err(InputError::IndexOutOfBounds { index: *index, arity });
    }
    Ok(())
}

// Splitting and combining expressions based on input hints

/// Splits items into input and output positions
///
/// Validates the hint against `items.len()`
pub fn split<Item>(
    hint: &InputHint,
    items: Vec<Item>,
) -> Result<(Vec<Item>, Vec<Item>), InputError> {
    validate(hint, items.len())?;
    let mut items_input = Vec::new();
    let mut items_output = Vec::new();
    // Inputs and outputs each keep source order
    for (index, item) in items.into_iter().enumerate() {
        if hint.indices.contains(&index) {
            items_input.push(item);
        } else {
            items_output.push(item);
        }
    }
    Ok((items_input, items_output))
}

/// Reconstructs source-order items from input and output positions
///
/// Validates the hint and both item counts.
pub fn combine<Item>(
    hint: &InputHint,
    items_input: Vec<Item>,
    items_output: Vec<Item>,
) -> Result<Vec<Item>, InputError> {
    // The hint must fit the combined length
    let input_actual = items_input.len();
    let output_actual = items_output.len();
    let items_len = input_actual + output_actual;
    validate(hint, items_len)?;
    let input_expected = hint.indices.len();
    let output_expected = items_len - input_expected;
    // Both lists must have the lengths the hint implies
    if input_actual != input_expected {
        return Err(InputError::InputCountMismatch {
            expected: input_expected,
            actual: input_actual,
        });
    }
    if output_actual != output_expected {
        return Err(InputError::OutputCountMismatch {
            expected: output_expected,
            actual: output_actual,
        });
    }

    // Walk source positions, drawing from whichever list owns each
    let mut items_input = items_input.into_iter();
    let mut items_output = items_output.into_iter();
    let mut items = Vec::with_capacity(items_len);
    // Refill source positions from the two lists in turn
    for idx in 0..items_len {
        let item = if hint.indices.contains(&idx) {
            items_input.next().ok_or(InputError::InputCountMismatch {
                expected: input_expected,
                actual: input_actual,
            })?
        } else {
            items_output.next().ok_or(InputError::OutputCountMismatch {
                expected: output_expected,
                actual: output_actual,
            })?
        };
        items.push(item);
    }
    Ok(items)
}

// Checking if a hint is conditional

/// Reports whether every item is an input
///
/// Validates the hint against `items.len()`
pub fn is_conditional<Item>(hint: &InputHint, items: &[Item]) -> Result<bool, InputError> {
    validate(hint, items.len())?;
    Ok(items
        .iter()
        .enumerate()
        .all(|(index, _)| hint.indices.contains(&index)))
}
