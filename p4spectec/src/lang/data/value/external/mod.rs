//! Value encoding to and from JSON, arena-relative or arena-independent
//!
//! Relative mode saves slot 7 as the number 7;
//! independent mode saves its contents.
//! The caller supplies the matching arena, lifetime, and encoding
//! for relative data.
//! Independent payloads are value trees (`tree`) that any arena can intern.
//! A case body is written as its filled notation in both modes,
//! so shape handles never appear in a payload.

use std::{rc::Rc, slice};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_derive_state::{DeserializeState, SerializeState};
use serde_state::{DeserializeState, SerializeState};

use crate::util::json::json;

use crate::lang::{
    common::{Id, prim::num::Number, source::Span},
    data::{
        intern::Interned,
        notation::{AtomPhrase, Mixop, Node, ShapeArena, ShapeKind},
        typ::TypKind,
    },
};

use super::{Arena, Value, ValueCase, ValueField, ValueKind, tree};

// = Configuration

/// Relative payloads belong to one live arena;
/// independent payloads carry contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    /// Handles as indices; readable only with the same arena.
    #[default]
    ArenaRelative,
    /// Full contents; readable anywhere.
    ArenaIndependent,
}

impl std::str::FromStr for Encoding {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "arena-relative" => Ok(Self::ArenaRelative),
            "arena-independent" => Ok(Self::ArenaIndependent),
            _ => Err("expected arena-relative or arena-independent".to_owned()),
        }
    }
}

impl std::fmt::Display for Encoding {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.write_str(match self {
            Self::ArenaRelative => "arena-relative",
            Self::ArenaIndependent => "arena-independent",
        })
    }
}

/// What an encoder needs: the encoding and the arena.
///
/// Relative handles are written as indices,
/// but a case body's notation is still expanded from the arena's shapes.
pub enum EncodeContext<'arena> {
    /// Write handles as indices.
    ArenaRelative(&'arena Arena),
    /// Resolve handles through this arena.
    ArenaIndependent(&'arena Arena),
}

impl<'arena> EncodeContext<'arena> {
    /// The context for an encoding.
    pub fn new(arena: &'arena Arena, encoding: Encoding) -> Self {
        match encoding {
            Encoding::ArenaRelative => Self::ArenaRelative(arena),
            Encoding::ArenaIndependent => Self::ArenaIndependent(arena),
        }
    }

    /// The arena handles are read from.
    fn arena(&self) -> &'arena Arena {
        match self {
            Self::ArenaRelative(arena) | Self::ArenaIndependent(arena) => arena,
        }
    }
}

/// What a decoder needs: the encoding and the arena.
///
/// Relative handles are read as indices,
/// but a case body's notation is still interned into the arena's shapes.
pub enum DecodeContext<'arena> {
    /// Read handles as indices.
    ArenaRelative(&'arena mut Arena),
    /// Intern contents into this arena.
    ArenaIndependent(&'arena mut Arena),
}

impl<'arena> DecodeContext<'arena> {
    /// The context for an encoding.
    pub fn new(arena: &'arena mut Arena, encoding: Encoding) -> Self {
        match encoding {
            Encoding::ArenaRelative => Self::ArenaRelative(arena),
            Encoding::ArenaIndependent => Self::ArenaIndependent(arena),
        }
    }

    /// The arena contents are interned into.
    fn arena_mut(&mut self) -> &mut Arena {
        match self {
            Self::ArenaRelative(arena) | Self::ArenaIndependent(arena) => arena,
        }
    }
}

// = Encode

// - Entry points

/// Keeps the arena-independent JSON and annotation contract.
pub fn encode<T>(arena: &Arena, data: &T) -> Result<json, serde_json::Error>
where
    T: for<'arena> SerializeState<EncodeContext<'arena>> + ?Sized,
{
    encode_with(arena, Encoding::ArenaIndependent, data)
}

/// Encodes with the chosen encoding.
pub fn encode_with<T>(
    arena: &Arena,
    encoding: Encoding,
    data: &T,
) -> Result<json, serde_json::Error>
where
    T: for<'arena> SerializeState<EncodeContext<'arena>> + ?Sized,
{
    let ctx = EncodeContext::new(arena, encoding);
    match encoding {
        // Relative payloads are shallow; a stack-growing serializer suffices
        Encoding::ArenaRelative => data
            .serialize_state(serde_stacker::Serializer::new(serde_json::value::Serializer), &ctx),
        // Conversion, serde, and destruction all recurse through the contents
        Encoding::ArenaIndependent => stacker::grow(32 * 1024 * 1024, || {
            data.serialize_state(serde_json::value::Serializer, &ctx)
        }),
    }
}

// - Interned values

impl SerializeState<EncodeContext<'_>> for Interned<ValueKind> {
    fn serialize_state<S: Serializer>(
        &self,
        serializer: S,
        ctx: &EncodeContext<'_>,
    ) -> Result<S::Ok, S::Error> {
        match ctx {
            // Relative: the index; independent: the body as a tree
            EncodeContext::ArenaRelative(_) => self.index().serialize(serializer),
            EncodeContext::ArenaIndependent(arena) => {
                tree::ValueKind::from_arena(arena, arena.value.values.get(*self))
                    .serialize(serializer)
            }
        }
    }
}

impl SerializeState<EncodeContext<'_>> for Interned<TypKind> {
    fn serialize_state<S: Serializer>(
        &self,
        serializer: S,
        ctx: &EncodeContext<'_>,
    ) -> Result<S::Ok, S::Error> {
        match ctx {
            EncodeContext::ArenaRelative(_) => self.index().serialize(serializer),
            EncodeContext::ArenaIndependent(arena) => {
                arena.value.types.get(*self).serialize(serializer)
            }
        }
    }
}

impl SerializeState<EncodeContext<'_>> for Interned<Span> {
    fn serialize_state<S: Serializer>(
        &self,
        serializer: S,
        ctx: &EncodeContext<'_>,
    ) -> Result<S::Ok, S::Error> {
        match ctx {
            EncodeContext::ArenaRelative(_) => self.index().serialize(serializer),
            EncodeContext::ArenaIndependent(arena) => {
                arena.value.spans.get(*self).serialize(serializer)
            }
        }
    }
}

// = Decode

// - Entry points

/// Decodes with the chosen encoding, interning into `arena` when independent.
pub fn decode_with<'de, T>(
    arena: &'de mut Arena,
    encoding: Encoding,
    json: &'de json,
) -> Result<T, serde_json::Error>
where
    T: DeserializeState<'de, DecodeContext<'de>>,
{
    let mut ctx = DecodeContext::new(arena, encoding);
    match encoding {
        // Relative payloads are shallow; a stack-growing deserializer suffices
        Encoding::ArenaRelative => {
            T::deserialize_state(&mut ctx, serde_stacker::Deserializer::new(json))
        }
        // Independent trees recurse deeply; grow the stack up front
        Encoding::ArenaIndependent => {
            stacker::grow(32 * 1024 * 1024, || T::deserialize_state(&mut ctx, json))
        }
    }
}

// - Interned values

impl<'de> DeserializeState<'de, DecodeContext<'_>> for Interned<ValueKind> {
    fn deserialize_state<D: Deserializer<'de>>(
        ctx: &mut DecodeContext<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        match ctx {
            // Relative: trust the index; independent: intern the tree
            DecodeContext::ArenaRelative(_) => u32::deserialize(deserializer).map(Self::from_index),
            DecodeContext::ArenaIndependent(arena) => {
                let kind = tree::ValueKind::deserialize(deserializer)?
                    .into_arena(arena)
                    .map_err(::serde::de::Error::custom)?;
                arena
                    .value
                    .values
                    .intern(kind, &arena.shape)
                    .map_err(::serde::de::Error::custom)
            }
        }
    }
}

impl<'de> DeserializeState<'de, DecodeContext<'_>> for Interned<TypKind> {
    fn deserialize_state<D: Deserializer<'de>>(
        ctx: &mut DecodeContext<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        match ctx {
            DecodeContext::ArenaRelative(_) => u32::deserialize(deserializer).map(Self::from_index),
            DecodeContext::ArenaIndependent(arena) => {
                let typ = TypKind::deserialize(deserializer)?.into();
                arena
                    .value
                    .types
                    .intern(typ)
                    .map_err(::serde::de::Error::custom)
            }
        }
    }
}

impl<'de> DeserializeState<'de, DecodeContext<'_>> for Interned<Span> {
    fn deserialize_state<D: Deserializer<'de>>(
        ctx: &mut DecodeContext<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        match ctx {
            DecodeContext::ArenaRelative(_) => u32::deserialize(deserializer).map(Self::from_index),
            DecodeContext::ArenaIndependent(arena) => {
                let span = Span::deserialize(deserializer)?;
                arena
                    .value
                    .spans
                    .intern(span)
                    .map_err(::serde::de::Error::custom)
            }
        }
    }
}

// = Value bodies

// Written through local enums rather than derived on the generic body,
// with the variant names and field shapes of the handle payload

// - Encode

impl SerializeState<EncodeContext<'_>> for ValueKind {
    fn serialize_state<S: Serializer>(
        &self,
        serializer: S,
        ctx: &EncodeContext<'_>,
    ) -> Result<S::Ok, S::Error> {
        #[derive(SerializeState)]
        #[serde(rename = "ValueKind")]
        #[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
        enum ValueKindRef<'a> {
            Bool(&'a bool),
            Num(&'a Number),
            Text(&'a String),
            Struct(#[serde(state)] &'a [ValueField]),
            Case(#[serde(state)] &'a ValueCase),
            Tuple(#[serde(state)] &'a [Value]),
            Opt(#[serde(state)] &'a Option<Value>),
            List(#[serde(state)] &'a [Value]),
            Func(#[serde(state)] &'a Id),
            Extern(&'a Rc<json>),
        }

        let kind = match self {
            Self::Bool(value) => ValueKindRef::Bool(value),
            Self::Num(num) => ValueKindRef::Num(num),
            Self::Text(text) => ValueKindRef::Text(text),
            Self::Struct(value_fields) => ValueKindRef::Struct(value_fields),
            Self::Case(value_case) => ValueKindRef::Case(value_case),
            Self::Tuple(values) => ValueKindRef::Tuple(values),
            Self::Opt(value) => ValueKindRef::Opt(value),
            Self::List(values) => ValueKindRef::List(values),
            Self::Func(id) => ValueKindRef::Func(id),
            Self::Extern(json) => ValueKindRef::Extern(json),
        };
        kind.serialize_state(serializer, ctx)
    }
}

// - Decode

impl<'de> DeserializeState<'de, DecodeContext<'de>> for ValueKind {
    fn deserialize_state<D: Deserializer<'de>>(
        ctx: &mut DecodeContext<'de>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        #[derive(DeserializeState)]
        #[serde(rename = "ValueKind")]
        #[serde(deserialize_state = "DecodeContext<'de>")]
        enum ValueKindOwned {
            Bool(bool),
            Num(Number),
            Text(String),
            Struct(#[serde(state)] Vec<ValueField>),
            Case(#[serde(state)] ValueCase),
            Tuple(#[serde(state)] Vec<Value>),
            Opt(#[serde(state)] Option<Value>),
            List(#[serde(state)] Vec<Value>),
            Func(#[serde(state)] Id),
            Extern(Rc<json>),
        }

        Ok(match ValueKindOwned::deserialize_state(ctx, deserializer)? {
            ValueKindOwned::Bool(value) => Self::Bool(value),
            ValueKindOwned::Num(num) => Self::Num(num),
            ValueKindOwned::Text(text) => Self::Text(text),
            ValueKindOwned::Struct(value_fields) => Self::Struct(value_fields),
            ValueKindOwned::Case(value_case) => Self::Case(value_case),
            ValueKindOwned::Tuple(values) => Self::Tuple(values),
            ValueKindOwned::Opt(value) => Self::Opt(value),
            ValueKindOwned::List(values) => Self::List(values),
            ValueKindOwned::Func(id) => Self::Func(id),
            ValueKindOwned::Extern(json) => Self::Extern(json),
        })
    }
}

// = Case bodies

// Both encodings write a case as its filled notation tree,
// with the variant names and shapes of the former `Mixfix`

/// A case written as its filled notation tree.
#[derive(SerializeState)]
#[serde(rename = "Mixfix")]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
enum CaseTree<'a> {
    Arg(#[serde(state)] &'a Value),
    Atom(#[serde(state)] &'a AtomPhrase),
    Brack(
        #[serde(state)] &'a AtomPhrase,
        #[serde(state)] Box<CaseTree<'a>>,
        #[serde(state)] &'a AtomPhrase,
    ),
    Infix(
        #[serde(state)] Box<CaseTree<'a>>,
        #[serde(state)] &'a AtomPhrase,
        #[serde(state)] Box<CaseTree<'a>>,
    ),
    Seq(#[serde(state)] Vec<CaseTree<'a>>),
}

/// A case read as its filled notation tree.
#[derive(DeserializeState)]
#[serde(rename = "Mixfix")]
#[serde(deserialize_state = "DecodeContext<'arena>", de_parameters = "'arena")]
enum CaseTreeOwned {
    Arg(#[serde(state)] Value),
    Atom(#[serde(state)] AtomPhrase),
    Brack(
        #[serde(state)] AtomPhrase,
        #[serde(state)] Box<CaseTreeOwned>,
        #[serde(state)] AtomPhrase,
    ),
    Infix(
        #[serde(state)] Box<CaseTreeOwned>,
        #[serde(state)] AtomPhrase,
        #[serde(state)] Box<CaseTreeOwned>,
    ),
    Seq(#[serde(state)] Vec<CaseTreeOwned>),
}

/// Fills a shape with arguments in notation order, borrowing its atoms.
fn case_tree<'a>(
    arena_shape: &'a ShapeArena,
    kind: &'a ShapeKind,
    values: &mut slice::Iter<'a, Value>,
) -> CaseTree<'a> {
    match kind {
        Node::Arg => CaseTree::Arg(values.next().expect("a case fills every position")),
        Node::Atom(atom) => CaseTree::Atom(atom),
        Node::Brack(atom_l, shape, atom_r) => {
            let tree = case_tree(arena_shape, arena_shape.kind(*shape), values);
            CaseTree::Brack(atom_l, Box::new(tree), atom_r)
        }
        Node::Infix(shape_l, atom, shape_r) => {
            let tree_l = case_tree(arena_shape, arena_shape.kind(*shape_l), values);
            let tree_r = case_tree(arena_shape, arena_shape.kind(*shape_r), values);
            CaseTree::Infix(Box::new(tree_l), atom, Box::new(tree_r))
        }
        Node::Seq(shapes) => CaseTree::Seq(
            shapes
                .iter()
                .map(|shape| case_tree(arena_shape, arena_shape.kind(*shape), values))
                .collect(),
        ),
    }
}

/// Splits a read tree into its mixop, moving arguments out in notation order.
fn split_case_tree(tree: CaseTreeOwned, values: &mut Vec<Value>) -> Mixop {
    match tree {
        CaseTreeOwned::Arg(value) => {
            values.push(value);
            Node::Arg
        }
        CaseTreeOwned::Atom(atom) => Node::Atom(atom),
        CaseTreeOwned::Brack(atom_l, tree, atom_r) => {
            Node::Brack(atom_l, Box::new(split_case_tree(*tree, values)), atom_r)
        }
        CaseTreeOwned::Infix(tree_l, atom, tree_r) => {
            let mixop_l = split_case_tree(*tree_l, values);
            let mixop_r = split_case_tree(*tree_r, values);
            Node::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
        }
        CaseTreeOwned::Seq(trees) => Node::Seq(
            trees
                .into_iter()
                .map(|tree| split_case_tree(tree, values))
                .collect(),
        ),
    }
}

// - Encode

impl SerializeState<EncodeContext<'_>> for ValueCase {
    fn serialize_state<S: Serializer>(
        &self,
        serializer: S,
        ctx: &EncodeContext<'_>,
    ) -> Result<S::Ok, S::Error> {
        // Write the filled notation, with arguments in the context's encoding
        let arena_shape = ctx.arena().arena_shape();
        let mut values = self.args().iter();
        case_tree(arena_shape, arena_shape.kind(*self.mixop()), &mut values)
            .serialize_state(serializer, ctx)
    }
}

// - Decode

impl<'de> DeserializeState<'de, DecodeContext<'_>> for ValueCase {
    fn deserialize_state<D: Deserializer<'de>>(
        ctx: &mut DecodeContext<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // Read the filled notation, with arguments in the context's encoding
        let tree = CaseTreeOwned::deserialize_state(ctx, deserializer)?;
        let mut values = Vec::new();
        let mixop = split_case_tree(tree, &mut values);
        let arena_shape = ctx.arena_mut().arena_shape_mut();
        let shape = arena_shape
            .intern(&mixop)
            .map_err(::serde::de::Error::custom)?;
        ValueCase::new_in(arena_shape, shape, values).map_err(::serde::de::Error::custom)
    }
}
