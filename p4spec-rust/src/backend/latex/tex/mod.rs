//! Semantic TeX documents and their interpretation
//!
//! `doc` preserves mathematical structure;
//! `layout::resolve` chooses concrete lines before `serialize::to_string` emits TeX.

pub(crate) mod doc;
pub(crate) mod layout;
pub(crate) mod link;
pub(crate) mod serialize;
pub(crate) mod width;

mod validate;
