//! Data shared by the language representations

pub mod ds;
pub mod notation;
pub mod source;
pub mod var;

pub use var::iter::Iter;
pub use var::{
    id::{Id, IdKind},
    tid::TId,
};
