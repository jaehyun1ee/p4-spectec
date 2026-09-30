//! Data shared by the internal language and its successors
//!
//! IL, AL, SL, and PL share one type language, one value representation,
//! and one notion of variable; EL has its own.
//! `intern` holds the handle-based storage these representations build on;
//! `shape` holds the notation shapes that case values refer to.

pub mod intern;
pub mod shape;
pub mod typ;
pub mod value;
pub mod var;
