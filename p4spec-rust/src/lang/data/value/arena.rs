//! Append-only storage for value bodies, types, and spans
//!
//! Handles belong to one arena; annotation changes preserve the stored body

use std::rc::Rc;

use super::{
    intern::{CanonId, CanonInterner, Interned, Interner, RcInterner},
    value::{Value, ValueError, ValueKind, ValueRef},
};
use crate::lang::{
    common::source::{NotePhrase, Span},
    data::typ::TypKind,
    xl::num,
};

// = Arena storage

#[derive(Debug)]
pub struct ValueArena {
    values: CanonInterner<ValueKind>,
    types: RcInterner<TypKind>,
    spans: Interner<Span>,
    span_empty: Interned<Span>,
    // Reuse scalar type allocations because RcInterner keys by address
    pub(super) typ_bool: Rc<TypKind>,
    pub(super) typ_nat: Rc<TypKind>,
    pub(super) typ_int: Rc<TypKind>,
    pub(super) typ_text: Rc<TypKind>,
}

impl Default for ValueArena {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueArena {
    // - Construction

    pub fn new() -> Self {
        let mut spans = Interner::new();
        let span_empty = spans
            .intern(Span::default())
            .expect("the first span fits in an interner index");
        Self {
            values: CanonInterner::new(),
            types: RcInterner::new(),
            spans,
            span_empty,
            typ_bool: Rc::new(TypKind::Bool),
            typ_nat: Rc::new(TypKind::Num(num::Typ::Nat)),
            typ_int: Rc::new(TypKind::Num(num::Typ::Int)),
            typ_text: Rc::new(TypKind::Text),
        }
    }

    // - Interning

    pub(super) fn alloc(
        &mut self,
        kind: ValueKind,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let node = self
            .values
            .intern(kind, ValueKind::hash_canon, ValueKind::eq_canon)?;
        let note = self.types.intern(typ)?;
        let span = self.intern_span(span)?;
        Ok(NotePhrase { node, note, span })
    }

    fn intern_span(&mut self, span: Span) -> Result<Interned<Span>, ValueError> {
        if span == *self.spans.get(self.span_empty) {
            return Ok(self.span_empty);
        }
        Ok(self.spans.intern(span)?)
    }

    // - Lookup

    pub fn kind(&self, value: &Value) -> &ValueKind {
        self.values.get(value.node)
    }

    pub fn canon_id(&self, value: &Value) -> CanonId<ValueKind> {
        self.values.canon_id(value.node)
    }

    pub fn typ(&self, value: &Value) -> &Rc<TypKind> {
        self.types.get(value.note)
    }

    pub fn span(&self, value: &Value) -> &Span {
        self.spans.get(value.span)
    }

    /// Borrows a value issued by this arena for syntax comparisons
    pub fn view(&self, value: Value) -> ValueRef<'_> {
        ValueRef { arena: self, value }
    }

    // - Annotations

    pub fn update_typ(&mut self, value: Value, typ: Rc<TypKind>) -> Result<Value, ValueError> {
        let note = self.types.intern(typ)?;
        Ok(Value { note, ..value })
    }

    pub fn update_span(&mut self, value: Value, span: Span) -> Result<Value, ValueError> {
        let span = self.intern_span(span)?;
        Ok(Value { span, ..value })
    }

    // - Printing

    pub fn to_string(&self, value: &Value) -> String {
        let mut output = String::new();
        let mut printer = crate::lang::traits::print::Printer::new(&mut output);
        crate::lang::il::print::print_value(self, value, &mut printer)
            .expect("writing to a String cannot fail");
        output
    }
}
