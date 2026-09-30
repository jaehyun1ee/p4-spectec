//! Value encoding to and from JSON, arena-relative or arena-independent
//!
//! Relative mode saves slot 7 as the number 7;
//! independent mode saves its contents.
//! The caller supplies the matching arena, lifetime, and encoding
//! for relative data.
//! Independent payloads are trees (`indep`) that any arena can intern.
//! A case body is written as its filled notation in both modes,
//! so shape handles never appear in a payload.
//! A stored body (`ValueKind`) is written with the variant names and shapes
//! of an externally tagged enum, children as the context encodes them.

pub mod indep;

use std::rc::Rc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_derive_state::{DeserializeState, SerializeState};
use serde_state::{DeserializeState, SerializeState};

use crate::util::json::json;

use crate::lang::{
    common::{Id, prim::num::Number, source::Span},
    data::{intern::Interned, notation::Mixfix, typ::TypKind},
};

use super::{Arena, Value, ValueCase, ValueField, ValueKind};

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

/// What an encoder needs: the encoding, and the arena where one is required.
///
/// Relative handles need no arena, but a case body does:
/// its notation is expanded from the arena's shapes.
pub enum EncodeContext<'arena> {
    /// Write handles as indices; case bodies cannot be written.
    ArenaRelative,
    /// Write handles as indices and expand case notation through this arena.
    ArenaRelativeWithArena(&'arena Arena),
    /// Resolve handles through this arena.
    ArenaIndependent(&'arena Arena),
}

impl<'arena> EncodeContext<'arena> {
    /// The context for an encoding, keeping the arena for case notation.
    pub fn new(arena: &'arena Arena, encoding: Encoding) -> Self {
        match encoding {
            Encoding::ArenaRelative => Self::ArenaRelativeWithArena(arena),
            Encoding::ArenaIndependent => Self::ArenaIndependent(arena),
        }
    }
}

/// What a decoder needs: the encoding, and the arena where one is required.
///
/// Relative handles need no arena, but a case body does:
/// its notation is interned into the arena's shapes.
pub enum DecodeContext<'arena> {
    /// Read handles as indices; case bodies cannot be read.
    ArenaRelative,
    /// Read handles as indices and intern case notation into this arena.
    ArenaRelativeWithArena(&'arena mut Arena),
    /// Intern contents into this arena.
    ArenaIndependent(&'arena mut Arena),
}

impl<'arena> DecodeContext<'arena> {
    /// The context for an encoding, keeping the arena for case notation.
    pub fn new(arena: &'arena mut Arena, encoding: Encoding) -> Self {
        match encoding {
            Encoding::ArenaRelative => Self::ArenaRelativeWithArena(arena),
            Encoding::ArenaIndependent => Self::ArenaIndependent(arena),
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
            EncodeContext::ArenaRelative | EncodeContext::ArenaRelativeWithArena(_) => {
                self.index().serialize(serializer)
            }
            EncodeContext::ArenaIndependent(arena) => {
                indep::ValueKind::from_arena(arena, arena.kind_of(*self)).serialize(serializer)
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
            EncodeContext::ArenaRelative | EncodeContext::ArenaRelativeWithArena(_) => {
                self.index().serialize(serializer)
            }
            EncodeContext::ArenaIndependent(arena) => arena.typ_of(*self).serialize(serializer),
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
            EncodeContext::ArenaRelative | EncodeContext::ArenaRelativeWithArena(_) => {
                self.index().serialize(serializer)
            }
            EncodeContext::ArenaIndependent(arena) => arena.span_of(*self).serialize(serializer),
        }
    }
}

// = Decode

// - Entry points

/// Decodes with the chosen encoding, interning contents when independent
/// and case notation in both modes.
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
            DecodeContext::ArenaRelative | DecodeContext::ArenaRelativeWithArena(_) => {
                u32::deserialize(deserializer).map(Self::from_index)
            }
            DecodeContext::ArenaIndependent(arena) => {
                let kind = indep::ValueKind::deserialize(deserializer)?
                    .into_arena(arena)
                    .map_err(::serde::de::Error::custom)?;
                arena.intern_kind(kind).map_err(::serde::de::Error::custom)
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
            DecodeContext::ArenaRelative | DecodeContext::ArenaRelativeWithArena(_) => {
                u32::deserialize(deserializer).map(Self::from_index)
            }
            DecodeContext::ArenaIndependent(arena) => {
                let typ = TypKind::deserialize(deserializer)?.into();
                arena.intern_typ(typ).map_err(::serde::de::Error::custom)
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
            DecodeContext::ArenaRelative | DecodeContext::ArenaRelativeWithArena(_) => {
                u32::deserialize(deserializer).map(Self::from_index)
            }
            DecodeContext::ArenaIndependent(arena) => {
                let span = Span::deserialize(deserializer)?;
                arena.intern_span(span).map_err(::serde::de::Error::custom)
            }
        }
    }
}

// = Value bodies

// Written through local enums rather than derived on the generic body,
// with the variant names and field shapes the former `ValueKind` derive had

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

// - Encode

impl SerializeState<EncodeContext<'_>> for ValueCase {
    fn serialize_state<S: Serializer>(
        &self,
        serializer: S,
        ctx: &EncodeContext<'_>,
    ) -> Result<S::Ok, S::Error> {
        // The notation is expanded from shapes, so an arena is required
        let arena = match ctx {
            EncodeContext::ArenaRelative => {
                return Err(::serde::ser::Error::custom(
                    "a case body needs an arena to expand its notation",
                ));
            }
            EncodeContext::ArenaRelativeWithArena(arena)
            | EncodeContext::ArenaIndependent(arena) => arena,
        };
        // Write the filled notation, with arguments in the context's encoding
        self.to_mixfix(arena.shapes())
            .serialize_state(serializer, ctx)
    }
}

// - Decode

impl<'de> DeserializeState<'de, DecodeContext<'_>> for ValueCase {
    fn deserialize_state<D: Deserializer<'de>>(
        ctx: &mut DecodeContext<'_>,
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // The notation is interned into shapes, so an arena is required
        if matches!(ctx, DecodeContext::ArenaRelative) {
            return Err(::serde::de::Error::custom(
                "a case body needs an arena to intern its notation",
            ));
        }
        // Read the filled notation, with arguments in the context's encoding
        let mixfix = Mixfix::deserialize_state(ctx, deserializer)?;
        let arena = match ctx {
            DecodeContext::ArenaRelative => unreachable!("rejected before reading"),
            DecodeContext::ArenaRelativeWithArena(arena)
            | DecodeContext::ArenaIndependent(arena) => arena,
        };
        ValueCase::from_mixfix(arena.shapes_mut(), mixfix).map_err(::serde::de::Error::custom)
    }
}
