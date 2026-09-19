//! Per-callable variable slots and copy-on-write value frames
//!
//! Layouts resolve names and iterator paths during preparation. Execution uses
//! slots; a reserved slot stays unbound until an assignment writes its value

use std::{collections::HashMap, fmt, rc::Rc};

use crate::lang::{
    common::{Id, Iter},
    data::{
        value::Value,
        var::{IdSlot, SlotIdx, Var, VarSlot},
    },
    traits::{
        eq::SyntaxEq,
        print::{Print, Printer},
    },
};

// == Frame layouts

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameLayout {
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

    fn reserve(&mut self, key: (String, Vec<Iter>)) -> SlotIdx {
        let slot_next = SlotIdx(self.slots.len());
        *self.slots.entry(key).or_insert(slot_next)
    }

    pub fn resolve_id(&mut self, id: Id) -> IdSlot {
        let slot = self.reserve((id.node.clone(), vec![]));
        IdSlot { id, slot }
    }

    pub fn resolve_var(&mut self, var: Var) -> VarSlot {
        let slot = self.reserve((var.id.node.clone(), var.iters.clone()));
        VarSlot { slot, var }
    }

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

#[derive(Clone, Debug, Default)]
pub struct Frame {
    layout: Rc<FrameLayout>,
    values: Rc<Vec<Option<Value>>>,
}

impl Frame {
    // - Construction

    pub fn new(layout: Rc<FrameLayout>) -> Self {
        let values = Rc::new(vec![None; layout.len()]);
        Self { layout, values }
    }

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

    pub fn set(&mut self, slot: SlotIdx, value: Value) {
        Rc::make_mut(&mut self.values)[slot.0] = Some(value);
    }
}

// == Callables

/// Callable syntax paired with its interpreter-owned local layout
#[derive(Clone, Debug, PartialEq)]
pub struct Callable<T> {
    pub def: T,
    pub layout: Rc<FrameLayout>,
}

impl<T: Print> Print for Callable<T> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.def.print(printer)
    }
}

impl<T: SyntaxEq> SyntaxEq for Callable<T> {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.def.syntax_eq(&other.def)
    }
}
