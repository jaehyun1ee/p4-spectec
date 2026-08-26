//! XL variable identifiers

use crate::domain::source::Spanned;

/// Applies strip var suffix
pub fn strip_var_suffix(id: &Spanned<String>) -> Spanned<String> {
    let underscore = id.node.find('_');
    let apostrophe = id.node.find('\'');
    let suffix_index = match (underscore, apostrophe) {
        (None, None) => return id.clone(),
        (Some(index), None) if id.node[index..].bytes().all(|byte| byte == b'_') => {
            return id.clone();
        }
        (Some(index), None) | (None, Some(index)) => index,
        (Some(index_l), Some(index_r)) => index_l.min(index_r),
    };
    crate::spanned! {
        node: id.node[..suffix_index].to_owned(),
        span: id,
    }
}
