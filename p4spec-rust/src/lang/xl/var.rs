//! XL variable identifiers

use crate::lang::el::ast::Id;

pub fn strip_var_suffix(id: &Id) -> Id {
    let underscore = id.node.find('_');
    let apostrophe = id.node.find('\'');
    let suffix_index = match (underscore, apostrophe) {
        (None, None) => return id.clone(),
        (Some(index), None) if id.node[index..].bytes().all(|byte| byte == b'_') => {
            return id.clone();
        }
        (Some(index), None) | (None, Some(index)) => index,
        (Some(left), Some(right)) => left.min(right),
    };
    id.clone().map(|name| name[..suffix_index].to_owned())
}
