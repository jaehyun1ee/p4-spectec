//! Identifiers shared by the language representations

use crate::lang::traits::print::{Print, Printer};

use crate::lang::common::source::Phrase;

/// Source-annotated identifier
pub type Id = Phrase<String>;

impl Print for Id {
    fn print(&self, printer: &mut Printer<'_>) -> std::fmt::Result {
        printer.write(&self.node)
    }
}

impl Id {
    /// Strips identifier suffixes while preserving the source span
    pub fn strip_suffix(&self) -> Self {
        crate::phrase! {
            node: strip_suffix(&self.node).to_owned(),
            span: self.span.clone(),
        }
    }
}

/// Strips identifier suffixes while preserving all-underscore tails
pub fn strip_suffix(id: &str) -> &str {
    let underscore = id.find('_');
    let apostrophe = id.find('\'');
    let suffix_index = match (underscore, apostrophe) {
        (None, None) => return id,
        (Some(index), None) if id[index..].bytes().all(|byte| byte == b'_') => {
            return id;
        }
        (Some(index), None) | (None, Some(index)) => index,
        (Some(index_l), Some(index_r)) => index_l.min(index_r),
    };
    &id[..suffix_index]
}
