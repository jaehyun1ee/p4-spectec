//! Notation nodes, generic over how a node holds its children
//!
//! `Node<A, R>` is one notation form:
//! atoms, argument holes of `A`, and children held as `R` chooses.
//! A lone child (a bracket or infix side) is an `R::Child`,
//! a child in a sequence an `R::Elem`;
//! `Tree` boxes lone children and keeps sequence elements in place,
//! which gives `Mixfix<T>` and `Mixop`.
//! `Repr::node` reaches the node behind a child,
//! so the traversals in `walk` are written once for every representation.

use std::borrow::Borrow;

use crate::lang::common::{notation::atom::Atom, source::Phrase};

// = Nodes

/// An atom paired with its source span.
pub type AtomPhrase = Phrase<Atom>;

/// How a notation node holds its children, and how to reach them.
///
/// A lone child always holds a sequence element,
/// so both kinds of children are reached through `node`.
pub trait Repr<A>: Sized + 'static {
    /// A child held alone: a bracket or infix side
    type Child: From<Self::Elem> + Borrow<Self::Elem>;
    /// A child in a sequence
    type Elem;
    /// The shape arena children are read from: none for trees
    type ShapeArena: ?Sized;

    /// The node behind a child.
    fn node<'a>(arena_shape: &'a Self::ShapeArena, elem: &'a Self::Elem) -> &'a Node<A, Self>
    where
        A: 'a;
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
    Seq(Vec<R::Elem>),
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

    /// The node behind a lone child.
    pub(crate) fn child<'a>(arena_shape: &'a R::ShapeArena, child: &'a R::Child) -> &'a Self
    where
        A: 'a,
    {
        R::node(arena_shape, Borrow::<R::Elem>::borrow(child))
    }
}
