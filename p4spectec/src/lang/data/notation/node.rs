//! Notation nodes, generic over how a node holds its children
//!
//! `Node<A, R>` is one notation form:
//! atoms, argument holes of `A`, and children held as `R` chooses.
//! `Tree` holds children in place, which gives `Mixfix<T>` and `Mixop`.
//! A representation also chooses the node's identity:
//! trees compare atoms by name, ignoring spans.

use crate::lang::common::{notation::atom::Atom, source::Phrase};

// = Nodes

/// An atom paired with its source span.
pub type AtomPhrase = Phrase<Atom>;

/// How a notation node holds its children.
pub trait Repr<A> {
    /// The child of a bracket or infix node.
    type Child;
    /// The children of a sequence node.
    type Children;
}

/// A notation node: atoms and argument holes, with children as `R` holds them.
///
/// For example `_ + _` is infix with two holes and `[ _ ]` brackets one.
/// Cloning, printing, comparison, hashing, and serialization are defined
/// per representation: a derive over `R::Child` would need a bound
/// on the node itself, which the trait solver cannot close.
pub enum Node<A, R: Repr<A>> {
    /// Argument position.
    Arg(A),
    /// Literal atom.
    Atom(AtomPhrase),
    /// Bracketed form.
    Brack(AtomPhrase, R::Child, AtomPhrase),
    /// Infix form.
    Infix(R::Child, AtomPhrase, R::Child),
    /// Sequence of forms.
    Seq(R::Children),
}

impl<A, R: Repr<A>> Node<A, R> {
    /// Orders the variants for comparison across forms.
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Self::Arg(_) => 0,
            Self::Atom(_) => 1,
            Self::Brack(..) => 2,
            Self::Infix(..) => 3,
            Self::Seq(_) => 4,
        }
    }
}
