use std::collections::BTreeMap;

/// Mirror table: clone session id to multicast group id
pub type Table = BTreeMap<usize, usize>;
