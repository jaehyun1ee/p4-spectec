//! Fixed case metadata within one exclusive arena borrow
//!
//! The first row follows ordinary case construction through every stage.
//! Later rows keep its type and shape while interning each body and span;
//! dropping the session discards the metadata before the arena is accessible.

use std::rc::Rc;

use smallvec::SmallVec;

use crate::lang::{
    common::source::Span,
    data::{
        intern::Interned,
        notation::{Mixop, Shape},
        typ::TypKind,
    },
};

use super::{Arena, Value, ValueError, ValueKind, arena::TypeNote, args::ValueParts, make};

/// Selects possible row bodies without asserting whole-map equivalence.
#[derive(PartialEq, Eq, Hash)]
pub(crate) struct CaseMapKey {
    value: Value,
    nodes: SmallVec<[Interned<ValueKind>; 4]>,
}

/// Reuses one successful case's metadata while its arena is borrowed exclusively.
pub(crate) struct CaseSession<'a> {
    arena: &'a mut Arena,
    note: Interned<TypKind>,
    shape: Shape,
}

impl<'a> CaseSession<'a> {
    /// Constructs the first row ordinarily before retaining its fixed metadata.
    pub(crate) fn start(
        arena: &'a mut Arena,
        typ: Rc<TypKind>,
        mixop: &Rc<Mixop>,
        values: &[Value],
    ) -> Result<(Self, Value), ValueError> {
        let value = make::case_from_slice(arena, typ, mixop, values, Span::default())?;
        let ValueKind::Case(value_case) = arena.kind(&value) else {
            unreachable!("case construction returns a case body");
        };
        let shape = *value_case.mixop();
        Ok((Self { arena, note: value.note, shape }, value))
    }

    /// Finds one prior output list after the first row has succeeded ordinarily.
    pub(crate) fn find_map_hint(
        &self,
        value: Value,
        values: &[Value],
    ) -> (CaseMapKey, Option<Interned<ValueKind>>) {
        let key = CaseMapKey { value, nodes: values.iter().map(|value| value.node).collect() };
        let node = self.arena.case_maps.get(&key).copied();
        (key, node)
    }

    /// Checks each row and validates its possible body before resolving the span.
    pub(crate) fn next(
        &mut self,
        values: &[Value],
        node_list: Option<Interned<ValueKind>>,
        idx: usize,
    ) -> Result<Value, ValueError> {
        // Preserve the original arity check before any body lookup
        assert_eq!(
            values.len(),
            self.arena.shape.arity(self.shape),
            "a mixfix fills every position"
        );
        // Every candidate body remains untrusted, even after a map-key hit
        let hint = node_list.and_then(|node| {
            let ValueKind::List(values) = self.arena.value.values.get(node) else {
                return None;
            };
            values.get(idx).map(|value| value.node)
        });
        self.arena.alloc_parts_note_hint(
            ValueParts::Case(self.shape, values),
            TypeNote::Known(self.note),
            Span::default(),
            hint,
            <[Value]>::to_vec,
        )
    }
}
