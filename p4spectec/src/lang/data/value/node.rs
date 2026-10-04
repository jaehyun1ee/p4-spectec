//! Value bodies, generic over how a body holds its children
//!
//! `ValueNode<R>` is one value body; `R: ValueRepr` chooses its children.
//! An option's content is an `R::Child`,
//! a field value, a tuple or list element, or a case argument an `R::Elem`,
//! and a case body is a mixop as `R::Mixop` holds it, with its arguments.
//! `Flat` holds arena handles and shapes (`ValueKind`);
//! `Tree` boxes an option's content and owns mixops (`tree::ValueKind`).
//! `map` and `try_map` convert a body between representations,
//! child by child in field and element order;
//! comparison and serialization are defined per representation.

use std::{borrow::Borrow, rc::Rc};

use crate::util::json::json;

use crate::lang::{
    common::{Id, notation::atom::Atom, prim::num::Number, source::Phrase},
    data::notation::Mixfix,
};

// = Bodies

/// How a value body holds its children.
///
/// A lone child always holds a sequence element,
/// boxed where a tree would otherwise contain itself.
pub trait ValueRepr {
    /// A child held alone: an option's content
    type Child: From<Self::Elem> + Borrow<Self::Elem>;
    /// A child in a sequence: a field value, a tuple or list element,
    /// or a case argument
    type Elem;
    /// A case body's mixop: a shape handle, or a tree
    type Mixop;
}

/// A value body with children as `R` holds them.
pub enum ValueNode<R: ValueRepr> {
    /// A boolean.
    Bool(bool),
    /// A natural or integer.
    Num(Number),
    /// A text.
    Text(String),
    /// Named fields in declaration order.
    Struct(Vec<(Phrase<Atom>, R::Elem)>),
    /// A variant case with its arguments.
    Case(Mixfix<R::Mixop, R::Elem>),
    /// A fixed-length tuple.
    Tuple(Vec<R::Elem>),
    /// An optional value.
    Opt(Option<R::Child>),
    /// A list.
    List(Vec<R::Elem>),
    /// A function, by name.
    Func(Id),
    /// A host-owned value, opaque to the specification.
    Extern(Rc<json>),
}

// - Tags

/// The kind of a value without its payload, for errors and ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueTag {
    Bool,
    Num,
    Text,
    Struct,
    Case,
    Tuple,
    Opt,
    List,
    Func,
    Extern,
}

impl<R: ValueRepr> ValueNode<R> {
    /// The kind of this body.
    pub(crate) fn tag(&self) -> ValueTag {
        match self {
            Self::Bool(_) => ValueTag::Bool,
            Self::Num(_) => ValueTag::Num,
            Self::Text(_) => ValueTag::Text,
            Self::Struct(_) => ValueTag::Struct,
            Self::Case(_) => ValueTag::Case,
            Self::Tuple(_) => ValueTag::Tuple,
            Self::Opt(_) => ValueTag::Opt,
            Self::List(_) => ValueTag::List,
            Self::Func(_) => ValueTag::Func,
            Self::Extern(_) => ValueTag::Extern,
        }
    }

    // - Conversion

    /// Copies the body into another representation, mapping each child.
    ///
    /// Elements are mapped in field and element order;
    /// primitives, atoms, function names, and host JSON are shared or cloned.
    pub fn map<S: ValueRepr>(
        &self,
        mut map_elem: impl FnMut(&R::Elem) -> S::Elem,
        map_case: impl FnOnce(&Mixfix<R::Mixop, R::Elem>) -> Mixfix<S::Mixop, S::Elem>,
    ) -> ValueNode<S> {
        match self {
            Self::Bool(value) => ValueNode::Bool(*value),
            Self::Num(num) => ValueNode::Num(num.clone()),
            Self::Text(text) => ValueNode::Text(text.clone()),
            Self::Struct(fields) => ValueNode::Struct(
                fields
                    .iter()
                    .map(|(atom, elem)| (atom.clone(), map_elem(elem)))
                    .collect(),
            ),
            Self::Case(case) => ValueNode::Case(map_case(case)),
            Self::Tuple(elems) => ValueNode::Tuple(elems.iter().map(map_elem).collect()),
            Self::Opt(child) => ValueNode::Opt(
                child
                    .as_ref()
                    .map(|child| S::Child::from(map_elem(Borrow::<R::Elem>::borrow(child)))),
            ),
            Self::List(elems) => ValueNode::List(elems.iter().map(map_elem).collect()),
            Self::Func(id) => ValueNode::Func(id.clone()),
            Self::Extern(json) => ValueNode::Extern(Rc::clone(json)),
        }
    }

    /// Moves the body into another representation, mapping each child.
    ///
    /// Children are mapped in field and element order, all through `ctx`,
    /// stopping at the first error.
    pub fn try_map<S: ValueRepr, C: ?Sized, E>(
        self,
        ctx: &mut C,
        mut map_elem: impl FnMut(&mut C, R::Elem) -> Result<S::Elem, E>,
        map_child: impl FnOnce(&mut C, R::Child) -> Result<S::Child, E>,
        map_case: impl FnOnce(&mut C, Mixfix<R::Mixop, R::Elem>) -> Result<Mixfix<S::Mixop, S::Elem>, E>,
    ) -> Result<ValueNode<S>, E> {
        Ok(match self {
            Self::Bool(value) => ValueNode::Bool(value),
            Self::Num(num) => ValueNode::Num(num),
            Self::Text(text) => ValueNode::Text(text),
            Self::Struct(fields) => ValueNode::Struct(
                fields
                    .into_iter()
                    .map(|(atom, elem)| Ok((atom, map_elem(ctx, elem)?)))
                    .collect::<Result<_, E>>()?,
            ),
            Self::Case(case) => ValueNode::Case(map_case(ctx, case)?),
            Self::Tuple(elems) => ValueNode::Tuple(
                elems
                    .into_iter()
                    .map(|elem| map_elem(ctx, elem))
                    .collect::<Result<_, E>>()?,
            ),
            Self::Opt(child) => {
                ValueNode::Opt(child.map(|child| map_child(ctx, child)).transpose()?)
            }
            Self::List(elems) => ValueNode::List(
                elems
                    .into_iter()
                    .map(|elem| map_elem(ctx, elem))
                    .collect::<Result<_, E>>()?,
            ),
            Self::Func(id) => ValueNode::Func(id),
            Self::Extern(json) => ValueNode::Extern(json),
        })
    }
}
