//! Prepared notations: a mixop with its interned shape
//!
//! Preparation interns each notation of a specification once;
//! evaluation builds and matches case values through the shape handle.
//! The shape is execution detail, so equality, syntax comparison,
//! and printing look at the mixop only, as for frame slots.

use std::{fmt, rc::Rc};

use crate::lang::{
    il::ast::NotationRef,
    traits::{
        eq::SyntaxEq,
        print::{Print, Printer},
    },
};

use super::{shape::Shape, tree::Mixop};

/// A notation with its shape in the specification's `ShapeArena`.
#[derive(Clone, Debug)]
pub struct MixopShape {
    /// The notation as written.
    pub mixop: Rc<Mixop>,
    /// Its shape, valid in the `ShapeArena` that preparation interned it into.
    pub shape: Shape,
}

impl PartialEq for MixopShape {
    fn eq(&self, other: &Self) -> bool {
        self.mixop == other.mixop
    }
}

impl SyntaxEq for MixopShape {
    fn syntax_eq(&self, other: &Self) -> bool {
        self.mixop.syntax_eq(&other.mixop)
    }
}

impl Print for MixopShape {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.mixop.print(printer)
    }
}

impl NotationRef for MixopShape {
    fn mixop(&self) -> &Mixop {
        &self.mixop
    }
}
