//! Ordered AL candidate selection and deterministic overlap checks

use crate::interp::shared::{
    backtrack::{Backtrack, err, ok},
    error::Error,
};

// = Sequential choice

pub fn choose_sequential<C, T>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(&C) -> Backtrack<T>,
) -> Backtrack<T> {
    let mut errors = Vec::new();
    for candidate in candidates {
        match evaluate(&candidate) {
            ok!(value) => return ok!(value),
            err!(errors) => return err!(errors),
            Backtrack::Unmatch(mut candidate_errors) => errors.append(&mut candidate_errors),
        }
    }
    Backtrack::Unmatch(errors)
}

// = Deterministic choice

pub fn choose_deterministic<C, T>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(&C) -> Backtrack<T>,
    nondet: impl FnOnce(C, C) -> Error,
) -> Backtrack<T> {
    let mut success = None;
    let mut errors = Vec::new();
    for candidate in candidates {
        match evaluate(&candidate) {
            ok!(value) => {
                if let Some((first, _)) = success {
                    return err!(vec![nondet(first, candidate)]);
                }
                success = Some((candidate, value));
                errors.clear();
            }
            err!(errors) => return err!(errors),
            Backtrack::Unmatch(mut candidate_errors) => {
                if success.is_none() {
                    errors.append(&mut candidate_errors);
                }
            }
        }
    }
    match success {
        Some((_, value)) => ok!(value),
        None => Backtrack::Unmatch(errors),
    }
}
