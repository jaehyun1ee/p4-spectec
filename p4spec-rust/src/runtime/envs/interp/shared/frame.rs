//! Per-callable variable slots and copy-on-write value frames
//!
//! Layouts resolve names and iterator paths during preparation.
//! Execution uses slots;
//! a reserved slot stays unbound until an assignment writes its value.
//! Frames share their value vector until one is written.

use std::{collections::HashMap, rc::Rc};

use crate::lang::{
    common::{Id, Iter},
    data::{
        value::Value,
        var::{IdSlot, SlotIdx, Var, VarSlot},
    },
};

// == Frame layouts

/// Slot assignment for one callable, keyed by name and iteration path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameLayout {
    /// Slot of each name under its iteration path.
    slots: HashMap<(String, Vec<Iter>), SlotIdx>,
}

impl FrameLayout {
    // - Accessors

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    // - Resolution

    /// The slot for a key, allocating the next one when the key is new.
    fn reserve(&mut self, key: (String, Vec<Iter>)) -> SlotIdx {
        let slot_next = SlotIdx(self.slots.len());
        *self.slots.entry(key).or_insert(slot_next)
    }

    /// Resolves a plain identifier to its slot.
    pub fn resolve_id(&mut self, id: Id) -> IdSlot {
        let slot = self.reserve((id.node.clone(), vec![]));
        IdSlot { id, slot }
    }

    /// Resolves a variable under its iteration path to its slot.
    pub fn resolve_var(&mut self, var: Var) -> VarSlot {
        let slot = self.reserve((var.id.node.clone(), var.iters.clone()));
        VarSlot { slot, var }
    }

    /// The slot of `var` one iteration deeper, resolved during preparation.
    pub fn find_iter_var(&self, var: &VarSlot, iter: Iter) -> VarSlot {
        let mut var = var.var.clone();
        var.iters.push(iter);
        let slot = *self
            .slots
            .get(&(var.id.node.clone(), var.iters.clone()))
            .expect("iterated binding is resolved during preparation");
        VarSlot { slot, var }
    }
}

// == Value frames

/// Values of one callable's slots, copy-on-write across clones.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Layout the slots follow.
    layout: Rc<FrameLayout>,
    /// Slot values, shared until written.
    values: Rc<Vec<Option<Value>>>,
}

impl Frame {
    // - Construction

    /// An all-unbound frame for the layout.
    pub fn new(layout: Rc<FrameLayout>) -> Self {
        let values = Rc::new(vec![None; layout.len()]);
        Self { layout, values }
    }

    /// A fresh frame with the same layout.
    pub fn wipe(&self) -> Self {
        Self::new(Rc::clone(&self.layout))
    }

    // - Accessors

    pub fn layout(&self) -> &Rc<FrameLayout> {
        &self.layout
    }

    pub fn get(&self, slot: SlotIdx) -> Option<&Value> {
        self.values[slot.0].as_ref()
    }

    // - Updates

    /// Writes a slot, copying the values first if they are shared.
    pub fn set(&mut self, slot: SlotIdx, value: Value) {
        Rc::make_mut(&mut self.values)[slot.0] = Some(value);
    }
}
