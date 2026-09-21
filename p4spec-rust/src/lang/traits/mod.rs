//! Traits shared across language representations
//!
//! `SyntaxEq` and `SyntaxCmp` compare nodes ignoring spans and notes,
//! `Free` collects identifiers, `Print` renders text;
//! each stage implements them for its model.

pub mod cmp;
pub mod eq;
pub mod free;
pub mod print;
