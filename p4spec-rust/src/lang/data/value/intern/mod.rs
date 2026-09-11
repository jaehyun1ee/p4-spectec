//! Typed handles and exact or canonical interning storage

// = Implementations

mod canon;
mod idx;
mod simple;

// = Public interface

pub use canon::{CanonId, CanonInterner};
pub use idx::Interned;
pub use simple::Interner;
