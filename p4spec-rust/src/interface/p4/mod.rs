//! P4 source frontend

pub mod context;
pub mod error;
pub mod extract;
pub mod lexer;
pub mod parse;
pub mod preprocessor;

mod tokens;
mod value;

#[allow(
    clippy::extra_unused_lifetimes,
    clippy::let_unit_value,
    clippy::type_complexity
)]
pub(crate) mod parser {
    include!(concat!(env!("OUT_DIR"), "/interface/p4/parser.rs"));
}
