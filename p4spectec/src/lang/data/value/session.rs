//! Fixed case metadata within one exclusive arena borrow
//!
//! The first row follows ordinary case construction through every stage.
//! Later rows keep its type and shape while interning each body and span;
//! dropping the session discards the metadata before the arena is accessible.

use std::rc::Rc;

use crate::lang::{
    common::source::Span,
    data::{
        intern::Interned,
        notation::{Mixop, Shape},
        typ::TypKind,
    },
};

use super::{Arena, Value, ValueError, ValueKind, arena::TypeNote, args::ValueParts, make};

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

    /// Checks each row and interns its body before resolving the span.
    pub(crate) fn next(&mut self, values: &[Value]) -> Result<Value, ValueError> {
        assert_eq!(
            values.len(),
            self.arena.shape.arity(self.shape),
            "a mixfix fills every position"
        );
        self.arena.alloc_parts_note(
            ValueParts::Case(self.shape, values),
            TypeNote::Known(self.note),
            Span::default(),
            <[Value]>::to_vec,
        )
    }
}
