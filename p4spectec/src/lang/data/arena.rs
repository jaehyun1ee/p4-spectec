//! Storage for a specification's notation shapes and a run's values
//!
//! An `Arena` holds a `MixopArena` beside the value storage.
//! The shapes are the specification's, shared with its prepared syntax,
//! and outlive `reset_values`; a case body's notation is one of them.
//! `value` allocates values into the arena and reads them back.

use std::rc::Rc;

use crate::lang::common::source::Span;

use super::{
    intern::CanonId,
    notation::MixopArena,
    typ::TypKind,
    value::{
        ValueArena, ValueError,
        flat::{Value, ValueKind, ValueRef},
    },
};

// = Arena storage

/// Storage for a specification's notation shapes and the values of one run.
///
/// Shapes survive `reset_values`; value, type, and span handles do not.
#[derive(Debug, Default)]
pub struct Arena {
    /// Notation shapes of prepared syntax and case bodies.
    pub(super) mixop: MixopArena,
    /// Value bodies, types, and spans.
    pub(super) value: ValueArena,
}

impl Arena {
    // - Construction

    /// An empty arena with the default span pre-interned.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty value store over shapes interned earlier.
    pub fn with_arena_mixop(arena_mixop: MixopArena) -> Self {
        Self { mixop: arena_mixop, value: ValueArena::default() }
    }

    /// Drops every value, type, and span, keeping the shapes.
    ///
    /// Value, type, and span handles issued before become invalid
    /// and numbering starts over; shape handles stay valid.
    pub fn reset_values(&mut self) {
        self.value = ValueArena::default();
    }

    // - Lookup

    /// The notation shapes of prepared syntax and case bodies.
    pub fn arena_mixop(&self) -> &MixopArena {
        &self.mixop
    }

    /// The notation shapes, for interning notations during a run.
    pub fn arena_mixop_mut(&mut self) -> &mut MixopArena {
        &mut self.mixop
    }
    // - Interning

    /// Interns the three parts and returns their handles as a value.
    pub(super) fn alloc(
        &mut self,
        kind: ValueKind,
        typ: Rc<TypKind>,
        span: Span,
    ) -> Result<Value, ValueError> {
        self.value.alloc(&self.mixop, kind, typ, span)
    }

    // - Lookup

    /// The body of a value.
    pub fn kind(&self, value: &Value) -> &ValueKind {
        self.value.kind(value.node)
    }

    /// The canonical identity of a value's body.
    pub fn canon_id(&self, value: &Value) -> CanonId<ValueKind> {
        self.value.canon_id(value.node)
    }

    /// The type of a value.
    pub fn typ(&self, value: &Value) -> &Rc<TypKind> {
        self.value.typ(value.note)
    }

    /// The span of a value.
    pub fn span(&self, value: &Value) -> &Span {
        self.value.span(value.span)
    }

    /// Borrows a value issued by this arena for syntax comparisons.
    pub fn view(&self, value: Value) -> ValueRef<'_> {
        ValueRef { arena: self, value }
    }

    // - Printing

    /// Prints a value in full, resolving its stored contents.
    pub fn to_string(&self, value: &Value) -> String {
        let mut output = String::new();
        let mut printer = crate::lang::traits::print::Printer::new(&mut output);
        super::value::print::print_value(self, value, &mut printer)
            .expect("writing to a String cannot fail");
        output
    }
}
