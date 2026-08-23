//! Input hints for relations

use crate::lang::el::ast::{Exp, ExpKind, Hole};

pub type T = Vec<i64>;

pub fn to_string(hint: &[i64]) -> String {
    format!(
        "hint(input {})",
        hint.iter()
            .map(|index| format!("%{index}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}
pub fn eq(left: &[i64], right: &[i64]) -> bool {
    left == right
}

pub fn init(hint_exp: &Exp) -> Option<T> {
    match &hint_exp.node {
        ExpKind::SeqE(hint_exps) => hint_exps
            .iter()
            .map(|hint_exp| match hint_exp.node {
                ExpKind::HoleE(Hole::Num(index)) => Some(index),
                _ => None,
            })
            .collect(),
        ExpKind::HoleE(Hole::Num(index)) => Some(vec![*index]),
        _ => None,
    }
}

pub fn validate(hint: &[i64], arity: i64) -> Result<(), String> {
    if hint.is_empty() {
        return Err("input hint is empty".to_owned());
    }
    if hint
        .iter()
        .enumerate()
        .any(|(index, item)| hint[..index].contains(item))
    {
        return Err("input hint contains duplicate indices".to_owned());
    }
    if hint.iter().any(|index| *index < 0 || *index >= arity) {
        return Err("input hint contains out-of-bounds indices".to_owned());
    }
    Ok(())
}

pub fn split<Item: Clone>(hint: &[i64], items: &[Item]) -> (Vec<Item>, Vec<Item>) {
    let mut items_input = Vec::new();
    let mut items_output = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if hint.contains(&(index as i64)) {
            items_input.push(item.clone());
        } else {
            items_output.push(item.clone());
        }
    }
    (items_input, items_output)
}

pub fn combine<Item>(hint: &[i64], items_input: Vec<Item>, items_output: Vec<Item>) -> Vec<Item> {
    let length = items_input.len() + items_output.len();
    let mut items_input = items_input.into_iter();
    let mut items_output = items_output.into_iter();
    let mut items = Vec::with_capacity(length);
    for index in 0..length {
        let item = if hint.contains(&(index as i64)) {
            items_input
                .next()
                .expect("input hint does not match input item count")
        } else {
            items_output
                .next()
                .expect("input hint does not match output item count")
        };
        items.push(item);
    }
    assert!(items_input.next().is_none());
    assert!(items_output.next().is_none());
    items
}

pub fn is_conditional<Item>(hint: &[i64], items: &[Item]) -> bool {
    items
        .iter()
        .enumerate()
        .all(|(index, _)| hint.contains(&(index as i64)))
}
