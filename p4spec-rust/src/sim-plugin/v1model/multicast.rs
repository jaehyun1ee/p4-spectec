use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Multicast node
pub struct Node {
    pub port: usize,
    pub rid: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Multicast state
pub struct State {
    pub handle_next: usize,
    /// Multicast group id to ordered node handles
    pub groups: BTreeMap<usize, Vec<usize>>,
    /// Node handle to ordered multicast nodes
    pub nodes: BTreeMap<usize, Vec<Node>>,
}

impl State {
    pub fn group_create(&mut self, group: usize) {
        self.groups.insert(group, vec![]);
    }

    pub fn node_create(&mut self, rid: usize, ports: &[usize]) {
        let handle = self.handle_next;
        self.handle_next = handle + 1;
        self.nodes
            .insert(handle, ports.iter().map(|port| Node { port: *port, rid }).collect());
    }

    pub fn node_associate(&mut self, group: usize, handle: usize) {
        if let Some(handles) = self.groups.get_mut(&group) {
            handles.insert(0, handle);
        }
    }
}
