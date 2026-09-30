//! Arena-independent value trees
//!
//! A tree contains its child values and annotations, without arena handles,
//! so it can be written to JSON and read into any arena.
//! `Indep` holds a body's children as trees;
//! `from_arena` and `into_arena` convert through `ValueKindF::map` and
//! `ValueKindF::try_map`, interning children before their parent.

use std::{fmt, rc::Rc};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::util::json::json;

use crate::lang::{
    common::{
        Id,
        notation::atom::Atom,
        prim::num::Number,
        source::{NotePhrase, Phrase},
    },
    data::{notation::Mixfix, typ::TypKind},
};

use super::super::{
    Arena, Value as ArenaValue, ValueCase as ArenaValueCase, ValueError,
    ValueKind as ArenaValueKind, ValueKindF, ValueRepr,
};

// == Types

/// Children held as trees, each with its type and span.
#[derive(Clone, Copy, Debug)]
pub struct Indep;

impl ValueRepr for Indep {
    type Elem = Value;
    type OptElem = Box<Value>;
    type Case = Mixfix<Box<Value>>;
    type Json = json;

    fn opt_into_elem(value: Box<Value>) -> Value {
        *value
    }
}

/// A value tree with its type; children are trees, not handles.
pub type Value = NotePhrase<ValueKind, TypKind>;

/// A value body whose children are trees.
pub type ValueKind = ValueKindF<Indep>;

// - Debugging

// The same text a derive prints: variant names and fields, no type name
impl fmt::Debug for ValueKind {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => fmt.debug_tuple("Bool").field(value).finish(),
            Self::Num(num) => fmt.debug_tuple("Num").field(num).finish(),
            Self::Text(text) => fmt.debug_tuple("Text").field(text).finish(),
            Self::Struct(fields) => fmt.debug_tuple("Struct").field(fields).finish(),
            Self::Case(mixfix) => fmt.debug_tuple("Case").field(mixfix).finish(),
            Self::Tuple(values) => fmt.debug_tuple("Tuple").field(values).finish(),
            Self::Opt(value) => fmt.debug_tuple("Opt").field(value).finish(),
            Self::List(values) => fmt.debug_tuple("List").field(values).finish(),
            Self::Func(id) => fmt.debug_tuple("Func").field(id).finish(),
            Self::Extern(json) => fmt.debug_tuple("Extern").field(json).finish(),
        }
    }
}

// == Serialization

// Written through local enums rather than derived on the generic body:
// a derive would bound the body on its own elements, which contain it,
// and the local enums keep the former derive's variant names and shapes

// - Encode

impl Serialize for ValueKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "ValueKind")]
        enum ValueKindRef<'a> {
            Bool(&'a bool),
            Num(&'a Number),
            Text(&'a String),
            Struct(&'a [(Phrase<Atom>, Value)]),
            Case(&'a Mixfix<Box<Value>>),
            Tuple(&'a [Value]),
            Opt(&'a Option<Box<Value>>),
            List(&'a [Value]),
            Func(&'a Id),
            Extern(&'a json),
        }

        let kind = match self {
            Self::Bool(value) => ValueKindRef::Bool(value),
            Self::Num(num) => ValueKindRef::Num(num),
            Self::Text(text) => ValueKindRef::Text(text),
            Self::Struct(fields) => ValueKindRef::Struct(fields),
            Self::Case(mixfix) => ValueKindRef::Case(mixfix),
            Self::Tuple(values) => ValueKindRef::Tuple(values),
            Self::Opt(value) => ValueKindRef::Opt(value),
            Self::List(values) => ValueKindRef::List(values),
            Self::Func(id) => ValueKindRef::Func(id),
            Self::Extern(json) => ValueKindRef::Extern(json),
        };
        kind.serialize(serializer)
    }
}

// - Decode

impl<'de> Deserialize<'de> for ValueKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "ValueKind")]
        enum ValueKindOwned {
            Bool(bool),
            Num(Number),
            Text(String),
            Struct(Vec<(Phrase<Atom>, Value)>),
            Case(Mixfix<Box<Value>>),
            Tuple(Vec<Value>),
            Opt(Option<Box<Value>>),
            List(Vec<Value>),
            Func(Id),
            Extern(json),
        }

        Ok(match ValueKindOwned::deserialize(deserializer)? {
            ValueKindOwned::Bool(value) => Self::Bool(value),
            ValueKindOwned::Num(num) => Self::Num(num),
            ValueKindOwned::Text(text) => Self::Text(text),
            ValueKindOwned::Struct(fields) => Self::Struct(fields),
            ValueKindOwned::Case(mixfix) => Self::Case(mixfix),
            ValueKindOwned::Tuple(values) => Self::Tuple(values),
            ValueKindOwned::Opt(value) => Self::Opt(value),
            ValueKindOwned::List(values) => Self::List(values),
            ValueKindOwned::Func(id) => Self::Func(id),
            ValueKindOwned::Extern(json) => Self::Extern(json),
        })
    }
}

// == Arena conversion

/// Copies an arena value into a tree, including its type and source span.
pub fn from_arena(arena: &Arena, value: &ArenaValue) -> Value {
    Value {
        node: ValueKind::from_arena(arena, arena.kind(value)),
        note: arena.typ(value).as_ref().clone(),
        span: arena.span(value).clone(),
    }
}

/// Interns the tree in the target arena, preserving its type and source span.
pub fn into_arena(arena: &mut Arena, value: Value) -> Result<ArenaValue, ValueError> {
    let kind = value.node.into_arena(arena)?;
    arena.alloc(kind, value.note.into(), value.span)
}

impl ValueKind {
    /// Expands child handles into trees and copies extern JSON unchanged.
    pub(super) fn from_arena(arena: &Arena, kind: &ArenaValueKind) -> Self {
        kind.map(
            |value| from_arena(arena, value),
            |value_case| {
                value_case
                    .to_mixfix(arena.shapes())
                    .map(|value| Box::new(from_arena(arena, value)))
            },
            |json| json.as_ref().clone(),
        )
    }

    /// Interns child trees in field and element order, keeping extern JSON.
    pub(super) fn into_arena(self, arena: &mut Arena) -> Result<ArenaValueKind, ValueError> {
        self.try_map(arena, into_arena, into_arena_case, |json| Ok(Rc::new(json)))
    }
}

/// Interns a case's arguments in notation order, then its notation.
fn into_arena_case(
    arena: &mut Arena,
    mixfix: Mixfix<Box<Value>>,
) -> Result<ArenaValueCase, ValueError> {
    let mixop = mixfix.to_mixop();
    let values = mixfix
        .into_args()
        .into_iter()
        .map(|value| into_arena(arena, *value))
        .collect::<Result<Vec<_>, _>>()?;
    ArenaValueCase::from_notation(arena.shapes_mut(), &mixop, values)
}
