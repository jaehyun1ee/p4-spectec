//! Input hints for relations

use crate::lang::el::ast::{Exp, ExpKind, Hole};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputHint {
    indices: Vec<i64>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InputError {
    #[error("input hint is empty")]
    Empty,

    #[error("input hint contains duplicate index {0}")]
    DuplicateIndex(i64),

    #[error("input hint index {index} is out of bounds for arity {arity}")]
    IndexOutOfBounds { index: i64, arity: usize },

    #[error("input hint expects {expected} input items, but got {actual}")]
    InputCountMismatch { expected: usize, actual: usize },

    #[error("input hint expects {expected} output items, but got {actual}")]
    OutputCountMismatch { expected: usize, actual: usize },
}

impl InputHint {
    pub fn new(indices: Vec<i64>) -> Self {
        Self { indices }
    }

    pub fn indices(&self) -> &[i64] {
        &self.indices
    }

    pub fn into_indices(self) -> Vec<i64> {
        self.indices
    }
}

pub fn to_string(hint: &InputHint) -> String {
    format!(
        "hint(input {})",
        hint.indices
            .iter()
            .map(|index| format!("%{index}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}
// Equivalence of hints

pub fn eq(left: &InputHint, right: &InputHint) -> bool {
    left == right
}

// Creating hints

pub fn init(hint_exp: &Exp) -> Option<InputHint> {
    let indices = match &hint_exp.node {
        ExpKind::SeqE(hint_exps) => hint_exps
            .iter()
            .map(|hint_exp| match hint_exp.node {
                ExpKind::HoleE(Hole::Num(index)) => Some(index),
                _ => None,
            })
            .collect(),
        ExpKind::HoleE(Hole::Num(index)) => Some(vec![*index]),
        _ => None,
    }?;
    Some(InputHint::new(indices))
}

// Validating hints

pub fn validate(hint: &InputHint, arity: usize) -> Result<(), InputError> {
    if hint.indices.is_empty() {
        return Err(InputError::Empty);
    }
    for (position, index) in hint.indices.iter().enumerate() {
        if hint.indices[..position].contains(index) {
            return Err(InputError::DuplicateIndex(*index));
        }
    }
    if let Some(index) = hint
        .indices
        .iter()
        .find(|index| **index < 0 || usize::try_from(**index).map_or(true, |index| index >= arity))
    {
        return Err(InputError::IndexOutOfBounds {
            index: *index,
            arity,
        });
    }
    Ok(())
}

// Splitting and combining expressions based on input hints

pub fn split<Item: Clone>(
    hint: &InputHint,
    items: &[Item],
) -> Result<(Vec<Item>, Vec<Item>), InputError> {
    validate(hint, items.len())?;
    let mut items_input = Vec::new();
    let mut items_output = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if hint.indices.contains(&(index as i64)) {
            items_input.push(item.clone());
        } else {
            items_output.push(item.clone());
        }
    }
    Ok((items_input, items_output))
}

pub fn combine<Item>(
    hint: &InputHint,
    items_input: Vec<Item>,
    items_output: Vec<Item>,
) -> Result<Vec<Item>, InputError> {
    let actual_input = items_input.len();
    let actual_output = items_output.len();
    let length = actual_input + actual_output;
    validate(hint, length)?;
    let expected_input = hint.indices.len();
    let expected_output = length - expected_input;
    if actual_input != expected_input {
        return Err(InputError::InputCountMismatch {
            expected: expected_input,
            actual: actual_input,
        });
    }
    if actual_output != expected_output {
        return Err(InputError::OutputCountMismatch {
            expected: expected_output,
            actual: actual_output,
        });
    }

    let mut items_input = items_input.into_iter();
    let mut items_output = items_output.into_iter();
    let mut items = Vec::with_capacity(length);
    for index in 0..length {
        let item = if hint.indices.contains(&(index as i64)) {
            items_input.next().ok_or(InputError::InputCountMismatch {
                expected: expected_input,
                actual: actual_input,
            })?
        } else {
            items_output.next().ok_or(InputError::OutputCountMismatch {
                expected: expected_output,
                actual: actual_output,
            })?
        };
        items.push(item);
    }
    Ok(items)
}

// Checking if a hint is conditional

pub fn is_conditional<Item>(hint: &InputHint, items: &[Item]) -> Result<bool, InputError> {
    validate(hint, items.len())?;
    Ok(items
        .iter()
        .enumerate()
        .all(|(index, _)| hint.indices.contains(&(index as i64))))
}
