//! Data shared by the internal language and its successors
//!
//! IL, AL, SL, and PL share one type language, one value representation,
//! and one notion of variable; EL has its own.
//! `intern` holds the handle-based storage these representations build on;
//! `notation` holds notation trees and the shapes case values refer to.
//! A representation lives in the lowest layer that uses it:
//! `common` serves EL and later, `data` serves IL and later.

pub mod intern;
pub mod notation;
pub mod typ;
pub mod value;
pub mod var;
