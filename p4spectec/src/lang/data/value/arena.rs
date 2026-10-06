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
        intern::{CanonId, CanonInterner, Interned, Interner, RcInterner},
        notation::MixopArena,
        typ::TypKind,
    },
};

use super::{
    error::ValueError,
    flat::{Value, ValueKind},
};

// = Value storage

/// Storage for value bodies, types, and spans.
#[derive(Debug)]
pub(in crate::lang::data) struct ValueArena {
    /// Bodies, with canonical identities; a case reads its shape's.
    values: CanonInterner<ValueKind>,
    /// Types, shared by allocation.
    types: RcInterner<TypKind>,
    /// Spans, shared by equality.
    spans: Interner<Span>,
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

impl ValueArena {
    /// Interns a value body and its annotations in allocation order.
    pub(in crate::lang::data) fn alloc(
        &mut self,
        arena_mixop: &MixopArena,
        value_kind: ValueKind,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        let node = self.intern_kind(value_kind, arena_mixop)?;
        let note = self.intern_typ(typ)?;
        let span = self.intern_span(span)?;
        Ok(Value { node, note, span })
    }

    /// Interns a body using the canonical identities of its mixops.
    pub(in crate::lang::data) fn intern_kind(
        &mut self,
        value_kind: ValueKind,
        arena_mixop: &MixopArena,
    ) -> Result<Interned<ValueKind>, std::num::TryFromIntError> {
        self.values.intern(value_kind, arena_mixop)
    }

    /// Interns a shared type annotation by allocation identity.
    pub(in crate::lang::data) fn intern_typ(
        &mut self,
        typ: Rc<TypKind>,
    ) -> Result<Interned<TypKind>, std::num::TryFromIntError> {
        self.types.intern(typ)
    }

    /// Interns an exact source location.
    pub(in crate::lang::data) fn intern_span(
        &mut self,
        span: Span,
    ) -> Result<Interned<Span>, std::num::TryFromIntError> {
        self.spans.intern(span)
    }

    /// Reads the body behind its handle.
    pub(in crate::lang::data) fn kind(&self, value: Interned<ValueKind>) -> &ValueKind {
        self.values.get(value)
    }

    /// Reads a type annotation.
    pub(in crate::lang::data) fn typ(&self, typ: Interned<TypKind>) -> &Rc<TypKind> {
        self.types.get(typ)
    }

    /// Reads a source location.
    pub(in crate::lang::data) fn span(&self, span: Interned<Span>) -> &Span {
        self.spans.get(span)
    }

    /// Returns the canonical identity of a body.
    pub(in crate::lang::data) fn canon_id(&self, value: Interned<ValueKind>) -> CanonId<ValueKind> {
        self.values.canon_id(value)
    }
}
