//! Append-only storage for notation shapes, value bodies, types, and spans
//!
//! An `Arena` holds a `ShapeArena` and a `ValueArena`;
//! handles belong to one arena; annotation changes preserve the stored body.
//! Bodies are interned canonically, types by `Rc` identity, spans exactly;
//! the default span is interned first so generated values share it.
//! `reset_values` drops every value, type, and span but keeps the shapes,
//! so a runner keeps the shapes its prepared specification refers to.

use std::rc::Rc;

use crate::lang::{
    common::source::Span,
    data::{
        intern::{CanonId, CanonInterner, Interned, Interner, RcInterner},
        shape::ShapeArena,
        typ::TypKind,
    },
};

use super::value::{Value, ValueError, ValueKind, ValueRef};

// = Arena storage

/// Storage for the shapes of one specification and the values of one run.
///
/// Shapes survive `reset_values`; value, type, and span handles do not.
#[derive(Debug, Default)]
pub struct Arena {
    /// Notation shapes of case values.
    shapes: ShapeArena,
    /// Value bodies, types, and spans.
    values: ValueArena,
}

/// Storage for value bodies, types, and spans.
#[derive(Debug)]
pub(super) struct ValueArena {
    /// Bodies, with canonical identities.
    values: CanonInterner<ValueKind>,
    /// Types, shared by allocation.
    types: RcInterner<TypKind>,
    /// Spans, shared by equality.
    spans: Interner<Span>,
}

impl Default for ValueArena {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueArena {
    /// An empty store with the default span pre-interned.
    fn new() -> Self {
        let mut spans = Interner::new();
        spans
            .intern_default()
            .expect("the first span fits in an interner index");
        Self { values: CanonInterner::new(), types: RcInterner::new(), spans }
    }
}

impl Arena {
    // - Construction

    /// An empty arena with the default span pre-interned.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty value store over shapes interned earlier.
    pub fn with_shapes(shapes: ShapeArena) -> Self {
        Self { shapes, values: ValueArena::new() }
    }

    /// Drops every value, type, and span, keeping the shapes.
    ///
    /// Value, type, and span handles issued before become invalid
    /// and numbering starts over; shape handles stay valid.
    pub fn reset_values(&mut self) {
        self.values = ValueArena::new();
    }

    // - Interning

    /// Interns the three parts and returns their handles as a value.
    pub(super) fn alloc(
        &mut self,
        kind: ValueKind,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let node = self.intern_kind(kind)?;
        let note = self.intern_typ(typ)?;
        let span = self.intern_span(span)?;
        Ok(Value { node, note, span })
    }

    /// Interns a body; a case body reads its shape's canonical identity.
    pub(super) fn intern_kind(
        &mut self,
        kind: ValueKind,
    ) -> Result<Interned<ValueKind>, ValueError> {
        Ok(self.values.values.intern(kind, &self.shapes)?)
    }

    /// Interns a type by allocation.
    pub(super) fn intern_typ(&mut self, typ: Rc<TypKind>) -> Result<Interned<TypKind>, ValueError> {
        Ok(self.values.types.intern(typ)?)
    }

    /// Interns a span by equality.
    pub(super) fn intern_span(&mut self, span: Span) -> Result<Interned<Span>, ValueError> {
        Ok(self.values.spans.intern(span)?)
    }

    // - Lookup

    /// The notation shapes of case values.
    pub fn shapes(&self) -> &ShapeArena {
        &self.shapes
    }

    /// The notation shapes, for interning notations during a run.
    pub fn shapes_mut(&mut self) -> &mut ShapeArena {
        &mut self.shapes
    }

    /// The body of a value.
    pub fn kind(&self, value: &Value) -> &ValueKind {
        self.kind_of(value.node)
    }

    /// The body behind a handle.
    pub(super) fn kind_of(&self, node: Interned<ValueKind>) -> &ValueKind {
        self.values.values.get(node)
    }

    /// The canonical identity of a value's body.
    pub fn canon_id(&self, value: &Value) -> CanonId<ValueKind> {
        self.values.values.canon_id(value.node)
    }

    /// The type of a value.
    pub fn typ(&self, value: &Value) -> &Rc<TypKind> {
        self.typ_of(value.note)
    }

    /// The type behind a handle.
    pub(super) fn typ_of(&self, note: Interned<TypKind>) -> &Rc<TypKind> {
        self.values.types.get(note)
    }

    /// The span of a value.
    pub fn span(&self, value: &Value) -> &Span {
        self.span_of(value.span)
    }

    /// The span behind a handle.
    pub(super) fn span_of(&self, span: Interned<Span>) -> &Span {
        self.values.spans.get(span)
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
