#[path = "value/arena.rs"]
mod arena;
#[path = "value/get.rs"]
mod get;
#[path = "value/intern.rs"]
mod intern;
#[path = "value/make.rs"]
mod make;
#[path = "value/serde.rs"]
mod serde;
#[path = "value/value.rs"]
#[allow(clippy::module_inception, reason = "mirror the value module layout")]
mod value;

use p4spec_rust::lang::common::source::{Position, Span};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

fn span(file: &str, line: i64) -> Span {
    Span::new(Position::new(file, line, 0), Position::new(file, line, 1))
}

fn hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
