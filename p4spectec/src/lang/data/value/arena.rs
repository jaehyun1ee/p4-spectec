//! Append-only storage for notation shapes, value bodies, types, and spans
//!
//! An `Arena` holds a `ShapeArena` beside a `ValueArena`;
//! handles belong to one arena; annotation changes preserve the stored body.
//! A case body's notation is a shape in the arena's `ShapeArena`.
//! Bodies are interned canonically, types by `Rc` identity, spans exactly;
//! the default span is interned first so generated values share it.

use std::{collections::HashMap, fmt, rc::Rc};

use foldhash::fast::RandomState;

use crate::lang::{
    common::source::Span,
    data::{
        intern::{CanonId, CanonInterner, Interned, Interner, RcInterner},
        notation::ShapeArena,
        typ::TypKind,
    },
};

use super::{
    args::ValueParts,
    error::ValueError,
    handle::{Value, ValueKind},
    primitive::Primitive,
    session::CaseMapKey,
    view::ValueRef,
};

/// Resolves a new annotation or reuses one retained by an exclusive session.
pub(super) enum TypeNote {
    Shared(Rc<TypKind>),
    Known(Interned<TypKind>),
}

// = Arena storage

/// Storage for the notation shapes and the values of one run.
#[derive(Default)]
pub struct Arena {
    /// Notation shapes of case bodies.
    pub(super) shape: ShapeArena,
    /// Value bodies, types, and spans.
    pub(super) value: ValueArena,
    /// Completed output lists used only as validated case-body hints.
    pub(super) case_maps: HashMap<CaseMapKey, Interned<ValueKind>, RandomState>,
}

impl fmt::Debug for Arena {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Arena")
            .field("shape", &self.shape)
            .field("value", &self.value)
            .finish()
    }
}

/// Storage for value bodies, types, and spans.
#[derive(Debug)]
pub(super) struct ValueArena {
    /// Bodies, with canonical identities; a case reads its shape's.
    pub(super) values: CanonInterner<ValueKind>,
    /// Types, shared by allocation.
    pub(super) types: RcInterner<TypKind>,
    /// Spans, shared by equality.
    pub(super) spans: Interner<Span>,
    /// Generated booleans after their first ordinary allocation.
    pub(super) values_bool: [Option<Value>; 2],
}

impl Default for ValueArena {
    fn default() -> Self {
        let mut spans = Interner::new();
        spans
            .intern_default()
            .expect("the first span fits in an interner index");
        Self {
            values: CanonInterner::new(),
            types: RcInterner::new(),
            spans,
            values_bool: [None; 2],
        }
    }
}

impl Arena {
    // - Construction

    /// An empty arena with the default span pre-interned.
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Interns composite arguments before materializing a stored body.
    pub(super) fn alloc_parts<Values: AsRef<[Value]>>(
        &mut self,
        parts: ValueParts<Values>,
        typ: Rc<TypKind>,
        span: Span,
        into_values: impl FnOnce(Values) -> Vec<Value>,
    ) -> Result<Value, ValueError> {
        self.alloc_parts_note(parts, TypeNote::Shared(typ), span, into_values)
    }

    /// Keeps body and span interning common when a session already owns the type.
    pub(super) fn alloc_parts_note<Values: AsRef<[Value]>>(
        &mut self,
        parts: ValueParts<Values>,
        note: TypeNote,
        span: Span,
        into_values: impl FnOnce(Values) -> Vec<Value>,
    ) -> Result<Value, ValueError> {
        self.alloc_parts_note_hint(parts, note, span, None, into_values)
    }

    /// Validates a body hint while preserving ordinary type and span stages.
    pub(super) fn alloc_parts_note_hint<Values: AsRef<[Value]>>(
        &mut self,
        parts: ValueParts<Values>,
        note: TypeNote,
        span: Span,
        hint: Option<Interned<ValueKind>>,
        into_values: impl FnOnce(Values) -> Vec<Value>,
    ) -> Result<Value, ValueError> {
        let node = self
            .value
            .values
            .intern_with_hint(parts, &self.shape, hint, |parts| {
                parts.into_kind(&self.shape, into_values)
            })?;
        let note = match note {
            TypeNote::Shared(typ) => self.value.types.intern(typ)?,
            TypeNote::Known(note) => note,
        };
        let span = self.value.spans.intern(span)?;
        Ok(Value { node, note, span })
    }

    /// Publishes hints only after the map's outer list has been interned.
    pub(crate) fn remember_case_map(&mut self, key: CaseMapKey, value: Value) {
        self.case_maps.insert(key, value.node);
    }

    /// Interns a borrowed primitive before cloning a new payload.
    pub(super) fn alloc_primitive(
        &mut self,
        primitive: Primitive<'_>,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        // Exact body hits still pass through ordinary type and span interning
        let node = self
            .value
            .values
            .intern_with(primitive, &self.shape, Primitive::into_kind)?;
        let note = self.value.types.intern(typ)?;
        let span = self.value.spans.intern(span)?;
        Ok(Value { node, note, span })
    }

    // - Lookup

    /// The notation shapes of case bodies.
    pub fn arena_shape(&self) -> &ShapeArena {
        &self.shape
    }

    /// The notation shapes, for interning notations during a run.
    pub fn arena_shape_mut(&mut self) -> &mut ShapeArena {
        &mut self.shape
    }

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
