//! Sets of notation-type patterns

use crate::lang::{
    il::ast,
    traits::{eq::SyntaxEq, print::Print},
};

use super::super::{AlgoError, AlgoErrorKind};

/// A source-insensitive set of notation types
#[derive(Clone, Debug, Default)]
pub struct PatternSet {
    elements: Vec<ast::NotTyp>,
}

impl PatternSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn contains(&self, not_typ: &ast::NotTyp) -> bool {
        self.elements.iter().any(|item| item.syntax_eq(not_typ))
    }

    pub fn insert(&mut self, not_typ: ast::NotTyp) -> bool {
        if self.contains(&not_typ) {
            return false;
        }
        self.elements.push(not_typ);
        self.elements.sort_by_key(Print::to_string);
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = &ast::NotTyp> {
        self.elements.iter()
    }

    pub fn intersection(&self, other: &Self) -> Self {
        self.iter()
            .filter(|not_typ| other.contains(not_typ))
            .cloned()
            .collect()
    }

    pub fn difference(&self, other: &Self) -> Self {
        self.iter()
            .filter(|not_typ| !other.contains(not_typ))
            .cloned()
            .collect()
    }
}

impl PartialEq for PatternSet {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|not_typ| other.contains(not_typ))
    }
}

impl Eq for PatternSet {}

impl FromIterator<ast::NotTyp> for PatternSet {
    fn from_iter<T: IntoIterator<Item = ast::NotTyp>>(not_typs: T) -> Self {
        let mut pattern_set = Self::new();
        for not_typ in not_typs {
            pattern_set.insert(not_typ);
        }
        pattern_set
    }
}

pub type PatternSets = Vec<PatternSet>;

fn check_arity(patterns_l: &PatternSets, patterns_r: &PatternSets) -> Result<(), AlgoError> {
    if patterns_l.len() == patterns_r.len() {
        return Ok(());
    }
    Err(AlgoError::new(
        AlgoErrorKind::PatternArityMismatch {
            expected: patterns_l.len(),
            actual: patterns_r.len(),
        },
        Default::default(),
    ))
}

pub fn has_overlap(patterns_l: &PatternSets, patterns_r: &PatternSets) -> Result<bool, AlgoError> {
    check_arity(patterns_l, patterns_r)?;
    Ok(patterns_l
        .iter()
        .zip(patterns_r)
        .all(|(pattern_l, pattern_r)| !pattern_l.intersection(pattern_r).is_empty()))
}

pub fn find_overlap(
    pattern_group: &[PatternSets],
) -> Result<Option<(&PatternSets, &PatternSets)>, AlgoError> {
    for (index, patterns) in pattern_group.iter().enumerate() {
        for patterns_other in &pattern_group[index + 1..] {
            if has_overlap(patterns, patterns_other)? {
                return Ok(Some((patterns, patterns_other)));
            }
        }
    }
    Ok(None)
}

pub fn subtract(
    patterns_total: &PatternSets,
    patterns: &PatternSets,
) -> Result<Vec<PatternSets>, AlgoError> {
    if !has_overlap(patterns_total, patterns)? {
        return Ok(vec![patterns_total.clone()]);
    }

    let mut fragments = Vec::new();
    let mut prefix = Vec::new();
    for (index, (pattern_total, pattern)) in patterns_total.iter().zip(patterns).enumerate() {
        let difference = pattern_total.difference(pattern);
        let intersection = pattern_total.intersection(pattern);
        if !difference.is_empty() {
            let mut fragment = prefix.clone();
            fragment.push(difference);
            fragment.extend_from_slice(&patterns_total[index + 1..]);
            fragments.push(fragment);
        }
        if intersection.is_empty() {
            break;
        }
        prefix.push(intersection);
    }
    Ok(fragments)
}

pub fn find_missing(
    patterns_total: &PatternSets,
    pattern_group: &[PatternSets],
) -> Result<Vec<PatternSets>, AlgoError> {
    let mut missing = vec![patterns_total.clone()];
    for patterns in pattern_group {
        let mut remaining = Vec::new();
        for patterns_total in &missing {
            remaining.extend(subtract(patterns_total, patterns)?);
        }
        missing = remaining;
    }
    Ok(missing)
}
