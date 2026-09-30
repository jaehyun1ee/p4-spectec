//! Value bodies, generic over how a body holds its children
//!
//! `ValueKindF<R>` is one value body; `R: ValueRepr` chooses how elements,
//! option contents, case bodies, and host JSON are held.
//! `Stored` holds handles into an `Arena` (`ValueKind`);
//! `Indep` holds trees that any arena can intern (`indep::ValueKind`).
//! `map` and `try_map` convert a body between representations,
//! child by child in field and element order;
//! comparison and serialization are defined per representation.

use std::borrow::Borrow;

use crate::lang::common::{Id, notation::atom::Atom, prim::num::Number, source::Phrase};

// = Bodies

/// How a value body holds its children.
pub trait ValueRepr {
    /// A struct field value, tuple element, or list element.
    type Elem;
    /// The content of an option.
    type OptElem: Borrow<Self::Elem> + From<Self::Elem>;
    /// The body of a variant case.
    type Case;
    /// A host-owned JSON value.
    type Json;

    /// Moves an option's content out as an element.
    fn opt_into_elem(opt_elem: Self::OptElem) -> Self::Elem;
}

/// A value body with children as `R` holds them.
pub enum ValueKindF<R: ValueRepr> {
    /// A boolean.
    Bool(bool),
    /// A natural or integer.
    Num(Number),
    /// A text.
    Text(String),
    /// Named fields in declaration order.
    Struct(Vec<(Phrase<Atom>, R::Elem)>),
    /// A variant case with its arguments.
    Case(R::Case),
    /// A fixed-length tuple.
    Tuple(Vec<R::Elem>),
    /// An optional value.
    Opt(Option<R::OptElem>),
    /// A list.
    List(Vec<R::Elem>),
    /// A function, by name.
    Func(Id),
    /// A host-owned value, opaque to the specification.
    Extern(R::Json),
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

impl<R: ValueRepr> ValueKindF<R> {
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
    /// primitives, atoms, and function names are cloned.
    pub fn map<S: ValueRepr>(
        &self,
        mut map_elem: impl FnMut(&R::Elem) -> S::Elem,
        map_case: impl FnOnce(&R::Case) -> S::Case,
        map_json: impl FnOnce(&R::Json) -> S::Json,
    ) -> ValueKindF<S> {
        match self {
            Self::Bool(value) => ValueKindF::Bool(*value),
            Self::Num(num) => ValueKindF::Num(num.clone()),
            Self::Text(text) => ValueKindF::Text(text.clone()),
            Self::Struct(fields) => ValueKindF::Struct(
                fields
                    .iter()
                    .map(|(atom, elem)| (atom.clone(), map_elem(elem)))
                    .collect(),
            ),
            Self::Case(case) => ValueKindF::Case(map_case(case)),
            Self::Tuple(elems) => ValueKindF::Tuple(elems.iter().map(map_elem).collect()),
            Self::Opt(opt_elem) => ValueKindF::Opt(
                opt_elem
                    .as_ref()
                    .map(|opt_elem| S::OptElem::from(map_elem(opt_elem.borrow()))),
            ),
            Self::List(elems) => ValueKindF::List(elems.iter().map(map_elem).collect()),
            Self::Func(id) => ValueKindF::Func(id.clone()),
            Self::Extern(json) => ValueKindF::Extern(map_json(json)),
        }
    }

    /// Moves the body into another representation, mapping each child.
    ///
    /// Elements are mapped in field and element order, all through `ctx`,
    /// stopping at the first error.
    pub fn try_map<S: ValueRepr, C: ?Sized, E>(
        self,
        ctx: &mut C,
        mut map_elem: impl FnMut(&mut C, R::Elem) -> Result<S::Elem, E>,
        map_case: impl FnOnce(&mut C, R::Case) -> Result<S::Case, E>,
        map_json: impl FnOnce(R::Json) -> Result<S::Json, E>,
    ) -> Result<ValueKindF<S>, E> {
        Ok(match self {
            Self::Bool(value) => ValueKindF::Bool(value),
            Self::Num(num) => ValueKindF::Num(num),
            Self::Text(text) => ValueKindF::Text(text),
            Self::Struct(fields) => ValueKindF::Struct(
                fields
                    .into_iter()
                    .map(|(atom, elem)| Ok((atom, map_elem(ctx, elem)?)))
                    .collect::<Result<_, E>>()?,
            ),
            Self::Case(case) => ValueKindF::Case(map_case(ctx, case)?),
            Self::Tuple(elems) => ValueKindF::Tuple(
                elems
                    .into_iter()
                    .map(|elem| map_elem(ctx, elem))
                    .collect::<Result<_, E>>()?,
            ),
            Self::Opt(opt_elem) => ValueKindF::Opt(
                opt_elem
                    .map(|opt_elem| map_elem(ctx, R::opt_into_elem(opt_elem)).map(S::OptElem::from))
                    .transpose()?,
            ),
            Self::List(elems) => ValueKindF::List(
                elems
                    .into_iter()
                    .map(|elem| map_elem(ctx, elem))
                    .collect::<Result<_, E>>()?,
            ),
            Self::Func(id) => ValueKindF::Func(id),
            Self::Extern(json) => ValueKindF::Extern(map_json(json)?),
        })
    }
}
