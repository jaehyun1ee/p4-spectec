//! Append-only storage for value bodies, types, and spans
//!
//! `ValueArena` is the value half of an `Arena`;
//! handles belong to one arena; annotation changes preserve the stored body.
//! Bodies are interned canonically, types by `Rc` identity, spans exactly;
//! the default span is interned first so generated values share it.

use std::rc::Rc;

use crate::lang::{
    common::source::Span,
    data::{
        arena::Arena,
        intern::{CanonId, CanonInterner, Interner, RcInterner},
        typ::TypKind,
    },
};

use super::{
    error::ValueError,
    flat::{Value, ValueKind},
    view::ValueRef,
};

// = Value storage

/// Storage for value bodies, types, and spans.
#[derive(Debug)]
pub(in crate::lang::data) struct ValueArena {
    /// Bodies, with canonical identities; a case reads its shape's.
    pub(super) values: CanonInterner<ValueKind>,
    /// Types, shared by allocation.
    pub(super) types: RcInterner<TypKind>,
    /// Spans, shared by equality.
    pub(super) spans: Interner<Span>,
}

impl Default for ValueArena {
    fn default() -> Self {
        let mut spans = Interner::new();
        spans
            .intern_default()
            .expect("the first span fits in an interner index");
        Self { values: CanonInterner::new(), types: RcInterner::new(), spans }
    }
}

impl Arena {
    // - Interning

    /// Interns the three parts and returns their handles as a value.
    pub(super) fn alloc(
        &mut self,
        kind: ValueKind,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let node = self.value.values.intern(kind, &self.shape)?;
        let note = self.value.types.intern(typ)?;
        let span = self.value.spans.intern(span)?;
        Ok(Value { node, note, span })
    }

    // - Lookup

    /// The body of a value.
    pub fn kind(&self, value: &Value) -> &ValueKind {
        self.value.values.get(value.node)
    }

    /// The canonical identity of a value's body.
    pub fn canon_id(&self, value: &Value) -> CanonId<ValueKind> {
        self.value.values.canon_id(value.node)
    }

    /// The type of a value.
    pub fn typ(&self, value: &Value) -> &Rc<TypKind> {
        self.value.types.get(value.note)
    }

    /// The span of a value.
    pub fn span(&self, value: &Value) -> &Span {
        self.value.spans.get(value.span)
    }

    /// Borrows a value issued by this arena for syntax comparisons.
    pub fn view(&self, value: Value) -> ValueRef<'_> {
        ValueRef { arena: self, value }
    }

    // - Printing

    /// Prints a value in full through the IL printer.
    pub fn to_string(&self, value: &Value) -> String {
        let mut output = String::new();
        let mut printer = crate::lang::traits::print::Printer::new(&mut output);
        crate::lang::il::print::print_value(self, value, &mut printer)
            .expect("writing to a String cannot fail");
        output
    }
}
