//! SpecTec source frontend

pub mod error;
pub mod lexer;

// G03-B2 will connect these parser helpers to the public parse entry points.
#[cfg_attr(not(test), allow(dead_code))]
mod parser_support;

#[allow(
    clippy::extra_unused_lifetimes,
    clippy::let_unit_value,
    clippy::type_complexity
)]
pub(crate) mod parser {
    include!(concat!(env!("OUT_DIR"), "/frontend/parser.rs"));
}

#[cfg(test)]
mod parser_tests;
