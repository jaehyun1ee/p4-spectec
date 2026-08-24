//! Field hints

use crate::lang::el::ast::{Exp, ExpKind, Text};

pub type T = Vec<Text>;

pub fn to_string(hint: &[Text]) -> String {
    format!(
        "hint(fields {})",
        hint.iter()
            .map(|text| crate::lang::el::print::string_of_text(text))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

// Creating hints

pub fn init(hint_exp: &Exp) -> Option<T> {
    match &hint_exp.node {
        ExpKind::TextE(text) => Some(vec![text.clone()]),
        ExpKind::SeqE(hint_exps) => hint_exps
            .iter()
            .map(|hint_exp| match &hint_exp.node {
                ExpKind::TextE(text) => Some(text.clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

// Validating hints

pub fn validate(hint: &[Text], arity: usize) -> Result<(), String> {
    if hint.len() == arity {
        Ok(())
    } else {
        Err(format!("expected {arity} strings, but got {}.", hint.len()))
    }
}
