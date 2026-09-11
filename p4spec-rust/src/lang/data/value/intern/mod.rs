//! Typed handles and structural, physical, or canonical interning storage

// = Implementations

mod canon;
mod idx;
mod rc;
mod simple;

// = Public interface

pub use canon::{CanonId, CanonInterner};
pub use idx::Interned;
pub use rc::RcInterner;
pub use simple::Interner;
