//! Data shared by the language representations

use crate::domain::source::Spanned;

pub mod ds;

/// Identifier text
pub type IdKind = String;

/// Source-annotated identifier
pub type Id = Spanned<IdKind>;
