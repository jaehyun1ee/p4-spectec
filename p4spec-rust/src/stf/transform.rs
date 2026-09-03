//! Target-specific normalizations used between STF parsing and simulation.
//!
//! Public transforms rewrite qualified-name components, validity markers, and
//! action qualification before simulation. For example, `hdr.$valid$` becomes
//! `hdr.isValid()`.

use super::ast::{Action, Match};

// == Public transforms

/// Rewrites the first qualified-name segment when it contains one of `substrings`.
pub fn rewrite_name_prefix(name: &str, substrings: &[&str], replacement: &str) -> String {
    let mut segments = name.split('.');
    let Some(head) = segments.next() else {
        return String::new();
    };
    let rewritten = if substrings
        .iter()
        .any(|substring| contains_ignore_ascii_case(head, substring))
    {
        replacement
    } else {
        head
    };
    std::iter::once(rewritten)
        .chain(segments)
        .collect::<Vec<_>>()
        .join(".")
}

/// Replaces the first case-insensitive occurrence of every requested substring.
pub fn replace_name_substrings(name: &str, substrings: &[&str], replacement: &str) -> String {
    substrings.iter().fold(name.to_owned(), |name, substring| {
        replace_first_ignore_ascii_case(&name, substring, replacement)
    })
}

pub fn rewrite_valid_match(name: &str) -> String {
    name.replace("$valid$", "isValid()")
}

pub fn transform_match((name, kind): Match) -> Match {
    (rewrite_valid_match(&name), kind)
}

pub fn unqualify_action(name: &str) -> String {
    name.rsplit('.').next().unwrap_or(name).to_owned()
}

pub fn transform_action(mut action: Action) -> Action {
    action.name = unqualify_action(&action.name);
    action
}

// == Case-insensitive string helpers

fn contains_ignore_ascii_case(value: &str, pattern: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&pattern.to_ascii_lowercase())
}

fn replace_first_ignore_ascii_case(value: &str, pattern: &str, replacement: &str) -> String {
    let value_lower = value.to_ascii_lowercase();
    let pattern_lower = pattern.to_ascii_lowercase();
    let Some(start) = value_lower.find(&pattern_lower) else {
        return value.to_owned();
    };
    let end = start + pattern.len();
    format!("{}{}{}", &value[..start], replacement, &value[end..])
}
