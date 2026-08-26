//! Data shared by the language representations

use self::source::Spanned;

pub mod ds;
pub mod notation;
pub mod noted;
pub mod source;

/// Identifier text
pub type IdKind = String;

/// Source-annotated identifier
pub type Id = Spanned<IdKind>;
