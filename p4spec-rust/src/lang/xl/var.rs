//! XL variable identifiers

use crate::lang::common::source::Phrase;

/// Applies strip var suffix
pub fn strip_var_suffix(id: &Phrase<String>) -> Phrase<String> {
    let stripped = strip_var_suffix_name(&id.node);
    if stripped.len() == id.node.len() {
        return id.clone();
    }
    crate::phrase! {
        node: stripped.to_owned(),
        span: id.span.clone(),
    }
}

pub(crate) fn strip_var_suffix_name(id: &str) -> &str {
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
