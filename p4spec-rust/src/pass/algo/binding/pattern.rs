//! Sets of notation-type patterns

use std::cmp::Ordering;

use crate::lang::{common::source::Span, il::ast, xl::num};

use super::super::{AlgoError, AlgoErrorKind};

fn compare_slices<T, U>(
    items_l: &[T],
    items_r: &[U],
    mut compare_item: impl FnMut(&T, &U) -> Ordering,
) -> Ordering {
    for (item_l, item_r) in items_l.iter().zip(items_r) {
        let ordering = compare_item(item_l, item_r);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    items_l.len().cmp(&items_r.len())
}

fn typ_tag(typ: &ast::TypKind) -> u8 {
    match typ {
        ast::TypKind::Bool => 0,
        ast::TypKind::Num(_) => 1,
        ast::TypKind::Text => 2,
        ast::TypKind::Var(_, _) => 3,
        ast::TypKind::Tuple(_) => 4,
        ast::TypKind::Iter(_, _) => 5,
        ast::TypKind::Func(_) => 6,
    }
}

fn compare_typ(typ_l: &ast::Typ, typ_r: &ast::Typ) -> Ordering {
    compare_typ_kind(&typ_l.node, &typ_r.node)
}

fn compare_typ_phrase(typ_l: &ast::Typ, typ_r: &ast::Typ) -> Ordering {
    compare_typ_kind(&typ_l.node, &typ_r.node).then_with(|| typ_l.span.cmp(&typ_r.span))
}

fn compare_typ_kind(typ_l: &ast::TypKind, typ_r: &ast::TypKind) -> Ordering {
    match (typ_l, typ_r) {
        (ast::TypKind::Bool, ast::TypKind::Bool) | (ast::TypKind::Text, ast::TypKind::Text) => {
            Ordering::Equal
        }
        (ast::TypKind::Num(num_typ_l), ast::TypKind::Num(num_typ_r)) => {
            num::compare_typ(*num_typ_l, *num_typ_r)
        }
        (ast::TypKind::Var(id_l, targs_l), ast::TypKind::Var(id_r, targs_r)) => id_l
            .cmp(id_r)
            .then_with(|| compare_slices(targs_l, targs_r, compare_typ_phrase)),
        (ast::TypKind::Tuple(typs_l), ast::TypKind::Tuple(typs_r)) => {
            compare_slices(typs_l, typs_r, compare_typ_phrase)
        }
        (ast::TypKind::Iter(typ_l, iter_l), ast::TypKind::Iter(typ_r, iter_r)) => {
            compare_typ_phrase(typ_l, typ_r).then_with(|| iter_l.cmp(iter_r))
        }
        (ast::TypKind::Func(func_l), ast::TypKind::Func(func_r)) => {
            compare_slices(&func_l.tparams, &func_r.tparams, |tparam_l, tparam_r| {
                tparam_l.cmp(tparam_r)
            })
            .then_with(|| {
                compare_slices(&func_l.typs_params, &func_r.typs_params, compare_typ_phrase)
            })
            .then_with(|| compare_typ_phrase(&func_l.typ_ret, &func_r.typ_ret))
        }
        _ => typ_tag(typ_l).cmp(&typ_tag(typ_r)),
    }
}

fn compare_not_typ(not_typ_l: &ast::NotTyp, not_typ_r: &ast::NotTyp) -> Ordering {
    let structure = not_typ_l
        .node
        .cmp_by(&not_typ_r.node, |_, _| Ordering::Equal);
    if structure != Ordering::Equal {
        return structure;
    }
    let typs_l = not_typ_l.node.args();
    let typs_r = not_typ_r.node.args();
    compare_slices(&typs_l, &typs_r, |typ_l, typ_r| compare_typ(typ_l, typ_r))
}

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
        self.elements
            .iter()
            .any(|item| compare_not_typ(item, not_typ) == Ordering::Equal)
    }

    pub fn insert(&mut self, not_typ: ast::NotTyp) -> bool {
        if self.contains(&not_typ) {
            return false;
        }
        self.elements.push(not_typ);
        self.elements.sort_by(compare_not_typ);
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

fn check_arity(
    span: &Span,
    patterns_l: &PatternSets,
    patterns_r: &PatternSets,
) -> Result<(), AlgoError> {
    if patterns_l.len() == patterns_r.len() {
        return Ok(());
    }
    Err(AlgoError::new(
        AlgoErrorKind::PatternArityMismatch {
            expected: patterns_l.len(),
            actual: patterns_r.len(),
        },
        span.clone(),
    ))
}

pub fn has_overlap(
    span: &Span,
    patterns_l: &PatternSets,
    patterns_r: &PatternSets,
) -> Result<bool, AlgoError> {
    check_arity(span, patterns_l, patterns_r)?;
    Ok(patterns_l
        .iter()
        .zip(patterns_r)
        .all(|(pattern_l, pattern_r)| !pattern_l.intersection(pattern_r).is_empty()))
}

pub fn find_overlap<'a>(
    span: &Span,
    pattern_group: &'a [PatternSets],
) -> Result<Option<(&'a PatternSets, &'a PatternSets)>, AlgoError> {
    for (index, patterns) in pattern_group.iter().enumerate() {
        for patterns_other in &pattern_group[index + 1..] {
            if has_overlap(span, patterns, patterns_other)? {
                return Ok(Some((patterns, patterns_other)));
            }
        }
    }
    Ok(None)
}

pub fn subtract(
    span: &Span,
    patterns_total: &PatternSets,
    patterns: &PatternSets,
) -> Result<Vec<PatternSets>, AlgoError> {
    if !has_overlap(span, patterns_total, patterns)? {
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
    span: &Span,
    patterns_total: &PatternSets,
    pattern_group: &[PatternSets],
) -> Result<Vec<PatternSets>, AlgoError> {
    let mut missing = vec![patterns_total.clone()];
    for patterns in pattern_group {
        let mut remaining = Vec::new();
        for patterns_total in &missing {
            remaining.extend(subtract(span, patterns_total, patterns)?);
        }
        missing = remaining;
    }
    Ok(missing)
}
