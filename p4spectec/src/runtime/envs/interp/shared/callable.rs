//! Prepared callable syntax and its local frame layout
//!
//! `Callable::prepare` runs the `Prepare` traversal once,
//! resolving every name to a slot and recording the layout its frames follow;
//! notations are interned into the specification's shapes on the way.

use std::rc::Rc;

use crate::lang::data::notation::ShapeArena;

use crate::interp::shared::prepare::{Prepare, PrepareContext};

use super::frame::FrameLayout;

/// Callable syntax paired with its interpreter-owned local layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Callable<T> {
    /// The prepared definition.
    pub def: T,
    /// Slot layout of the definition's frames.
    pub layout: Rc<FrameLayout>,
}

impl<T> Callable<T> {
    /// Prepares a definition, collecting its slot layout.
    ///
    /// Notations are interned into `arena_shape`,
    /// which must stay with the prepared definition for evaluation.
    pub fn prepare<S: Prepare<Output = T>>(source: S, arena_shape: &mut ShapeArena) -> Self {
        // The traversal fills the layout as it resolves names
        let mut layout = FrameLayout::default();
        let def = source.prepare(&mut PrepareContext { layout: &mut layout, arena_shape });
        Self { def, layout: Rc::new(layout) }
    }
}
