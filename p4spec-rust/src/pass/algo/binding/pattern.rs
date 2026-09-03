//! Pattern-set operations for table exclusiveness and exhaustiveness
//!
//! `PatternSet` contains the notation-type alternatives for one argument, while
//! `PatternSets` is their Cartesian product across one table row

use crate::lang::{
    common::{ds::set::PhraseSet, source::Span},
    il::ast,
};

use super::super::{AlgoError, AlgoErrorKind};

// == Pattern sets

// - Single argument

/// A notation-type set compared by syntax
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternSet(PhraseSet<ast::NotTyp>);

impl PatternSet {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn intersection(&self, other: &Self) -> Self {
        let not_typs = self.0.intersection(&other.0);
        Self(not_typs)
    }

    fn difference(&self, other: &Self) -> Self {
        let not_typs = self.0.difference(&other.0);
        Self(not_typs)
    }
}

impl FromIterator<ast::NotTyp> for PatternSet {
    fn from_iter<T: IntoIterator<Item = ast::NotTyp>>(not_typs: T) -> Self {
        let not_typs = not_typs.into_iter().collect();
        Self(not_typs)
    }
}

// - Table row

/// Ordered pattern sets for the arguments of one table row
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternSets(Vec<PatternSet>);

impl FromIterator<PatternSet> for PatternSets {
    fn from_iter<T: IntoIterator<Item = PatternSet>>(pattern_sets: T) -> Self {
        let pattern_sets = pattern_sets.into_iter().collect();
        Self(pattern_sets)
    }
}

// == Exclusiveness checks

fn check_arity(
    span: &Span,
    pattern_sets_l: &PatternSets,
    pattern_sets_r: &PatternSets,
) -> Result<(), AlgoError> {
    let expected = pattern_sets_l.0.len();
    let actual = pattern_sets_r.0.len();
    if expected == actual {
        return Ok(());
    }
    Err(AlgoError::new(
        AlgoErrorKind::PatternArityMismatch { expected, actual },
        span.clone(),
    ))
}

pub fn has_overlap(
    span: &Span,
    pattern_sets_l: &PatternSets,
    pattern_sets_r: &PatternSets,
) -> Result<bool, AlgoError> {
    check_arity(span, pattern_sets_l, pattern_sets_r)?;
    let has_overlap =
        pattern_sets_l
            .0
            .iter()
            .zip(&pattern_sets_r.0)
            .all(|(pattern_set_l, pattern_set_r)| {
                let intersection = pattern_set_l.intersection(pattern_set_r);
                !intersection.is_empty()
            });
    Ok(has_overlap)
}

pub fn find_overlap<'a>(
    span: &Span,
    pattern_sets_group: &'a [PatternSets],
) -> Result<Option<(&'a PatternSets, &'a PatternSets)>, AlgoError> {
    for (index, pattern_sets) in pattern_sets_group.iter().enumerate() {
        for pattern_sets_other in &pattern_sets_group[index + 1..] {
            if has_overlap(span, pattern_sets, pattern_sets_other)? {
                let overlap = (pattern_sets, pattern_sets_other);
                return Ok(Some(overlap));
            }
        }
    }
    Ok(None)
}

// == Exhaustiveness checks

pub fn subtract(
    span: &Span,
    pattern_sets_total: &PatternSets,
    pattern_sets: &PatternSets,
) -> Result<Vec<PatternSets>, AlgoError> {
    if !has_overlap(span, pattern_sets_total, pattern_sets)? {
        let pattern_sets_total = pattern_sets_total.clone();
        return Ok(vec![pattern_sets_total]);
    }

    // F × F' − W × W' = (F − W) × F' ∪ (F ∩ W) × (F' − W')
    let mut pattern_sets_group_fragment = Vec::new();
    let pattern_sets_prefix = Vec::new();
    let mut pattern_sets_prefix = PatternSets(pattern_sets_prefix);
    for (index, (pattern_set_total, pattern_set)) in
        pattern_sets_total.0.iter().zip(&pattern_sets.0).enumerate()
    {
        let pattern_set_diff = pattern_set_total.difference(pattern_set);
        let pattern_set_inter = pattern_set_total.intersection(pattern_set);

        // (F − W) × F'
        if !pattern_set_diff.is_empty() {
            let mut pattern_sets_fragment = pattern_sets_prefix.clone();
            pattern_sets_fragment.0.push(pattern_set_diff);
            pattern_sets_fragment
                .0
                .extend_from_slice(&pattern_sets_total.0[index + 1..]);
            pattern_sets_group_fragment.push(pattern_sets_fragment);
        }

        // (F ∩ W) × (F' − W')
        if pattern_set_inter.is_empty() {
            break;
        }
        pattern_sets_prefix.0.push(pattern_set_inter);
    }
    Ok(pattern_sets_group_fragment)
}

pub fn find_missing(
    span: &Span,
    pattern_sets_total: &PatternSets,
    pattern_sets_group: &[PatternSets],
) -> Result<Vec<PatternSets>, AlgoError> {
    let mut pattern_sets_group_missing = vec![pattern_sets_total.clone()];
    for pattern_sets in pattern_sets_group {
        let mut pattern_sets_group_remaining = Vec::new();
        for pattern_sets_total in &pattern_sets_group_missing {
            let pattern_sets_group_fragment = subtract(span, pattern_sets_total, pattern_sets)?;
            pattern_sets_group_remaining.extend(pattern_sets_group_fragment);
        }
        pattern_sets_group_missing = pattern_sets_group_remaining;
    }
    Ok(pattern_sets_group_missing)
}
