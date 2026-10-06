//! Storage for a specification's notation shapes and a run's values
//!
//! An `Arena` holds a `MixopArena` beside the value storage.
//! The shapes are the specification's, shared with its prepared syntax,
//! and outlive `reset_values`; a case body's notation is one of them.
//! `value` allocates values into the arena and reads them back.

use super::{notation::MixopArena, value::ValueArena};

// = Arena storage

/// Storage for a specification's notation shapes and the values of one run.
///
/// Shapes survive `reset_values`; value, type, and span handles do not.
#[derive(Debug, Default)]
pub struct Arena {
    /// Notation shapes of prepared syntax and case bodies.
    pub(super) mixop: MixopArena,
    /// ValueFlat bodies, types, and spans.
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
    /// ValueFlat, type, and span handles issued before become invalid
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
}
