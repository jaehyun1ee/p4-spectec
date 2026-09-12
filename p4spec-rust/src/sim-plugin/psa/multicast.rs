use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Multicast node
pub struct Node {
    pub port: i64,
    pub instance: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Multicast state
pub struct State {
    pub handle_next: i64,
    /// Multicast group id to ordered node handles
    pub groups: BTreeMap<i64, Vec<i64>>,
    /// Node handle to ordered multicast nodes
    pub nodes: BTreeMap<i64, Vec<Node>>,
}

impl State {
    pub fn group_create(&mut self, group: i64) {
        self.groups.insert(group, vec![]);
    }
    pub fn node_create(&mut self, instance: i64, ports: &[i64]) {
        let handle = self.handle_next;
        self.handle_next = handle.wrapping_add(1).wrapping_shl(1) >> 1;
        self.nodes.insert(
            handle,
            ports
                .iter()
                .map(|port| Node {
                    port: *port,
                    instance,
                })
                .collect(),
        );
    }
    pub fn node_associate(&mut self, group: i64, handle: i64) {
        if let Some(handles) = self.groups.get_mut(&group) {
            handles.push(handle);
        }
    }
}
