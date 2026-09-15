use std::collections::BTreeMap;

/// Mirror table: clone session id to multicast group id
pub type Table = BTreeMap<i64, i64>;
