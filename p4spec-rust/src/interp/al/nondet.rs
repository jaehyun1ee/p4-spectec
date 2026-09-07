//! Choice that requires exactly one successful AL candidate
//!
//! `choose_deterministic` keeps checking after the first success. A second
//! success identifies both candidates; a fatal failure stops evaluation even
//! after a success. Candidate identities travel with their evaluation inputs,
//! avoiding separate lists whose lengths could disagree.

use super::backtrack::{Backtrack, FailTrace};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BacktrackDet<T, C> {
    Ok(T),
    Err(Vec<FailTrace>),
    Unmatch(Vec<FailTrace>),
    Nondet(C, C),
}

impl<T, C> From<Backtrack<T>> for BacktrackDet<T, C> {
    fn from(result: Backtrack<T>) -> Self {
        match result {
            Backtrack::Ok(value) => Self::Ok(value),
            Backtrack::Err(traces) => Self::Err(traces),
            Backtrack::Unmatch(traces) => Self::Unmatch(traces),
        }
    }
}

pub fn choose_deterministic<C, T>(
    candidates: impl IntoIterator<Item = C>,
    mut evaluate: impl FnMut(&C) -> Backtrack<T>,
) -> BacktrackDet<T, C> {
    let mut success = None;
    let mut traces = Vec::new();
    for candidate in candidates {
        match evaluate(&candidate) {
            Backtrack::Ok(value) => {
                if let Some((candidate_first, _)) = success {
                    return BacktrackDet::Nondet(candidate_first, candidate);
                }
                success = Some((candidate, value));
                traces.clear();
            }
            Backtrack::Err(traces) => return BacktrackDet::Err(traces),
            Backtrack::Unmatch(mut traces_candidate) => {
                if success.is_none() {
                    traces.append(&mut traces_candidate);
                }
            }
        }
    }
    match success {
        Some((_, value)) => BacktrackDet::Ok(value),
        None => BacktrackDet::Unmatch(traces),
    }
}
