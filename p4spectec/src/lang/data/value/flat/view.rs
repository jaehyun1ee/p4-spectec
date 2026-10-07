//! Borrowed views of flat values
//!
//! `ValueRef` pairs a value with the arena used to compare its contents.

use crate::lang::data::arena::Arena;

use super::Value;

// = Borrowed views

/// A value together with its arena, for comparisons that must read bodies.
#[derive(Clone, Copy, Debug)]
pub struct ValueRef<'a> {
    pub(in crate::lang::data) arena: &'a Arena,
    pub(in crate::lang::data) value: Value,
}
