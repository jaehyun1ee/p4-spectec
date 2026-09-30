//! Stages of IL syntax: what differs between source and prepared forms
//!
//! A `Stage` chooses identifier and variable occurrences and how a notation
//! expression refers to its notation (`NotationRef`).
//! `Source` uses plain `Id`, `Var`, and an `Rc<Mixop>`;
//! the interpreters prepare syntax into a stage with frame slots and shapes.
//! A stage chooses parts of syntax, where `data::notation::Repr`
//! chooses how one data node holds its children.

use std::{fmt, rc::Rc};

use super::ast::{Id, Mixop, Var};

/// A notation as a stage refers to it, exposing its mixop.
pub trait NotationRef: Clone + fmt::Debug + PartialEq {
    /// The notation's atoms and argument positions.
    fn mixop(&self) -> &Mixop;
}

/// The parts of syntax that differ between source and prepared forms.
///
/// Printing, syntax equality, and free identifiers read a notation
/// through `NotationRef::mixop`, so they behave alike at every stage.
pub trait Stage: Clone + fmt::Debug + PartialEq {
    /// Identifier occurrences.
    type Id: Clone + fmt::Debug + PartialEq;
    /// Variable occurrences.
    type Var: Clone + fmt::Debug + PartialEq;
    /// Notations of notation expressions.
    type Notation: NotationRef;
}

/// Syntax as elaboration and the passes produce it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source;

impl Stage for Source {
    type Id = Id;
    type Var = Var;
    type Notation = Rc<Mixop>;
}

impl NotationRef for Rc<Mixop> {
    fn mixop(&self) -> &Mixop {
        self
    }
}
