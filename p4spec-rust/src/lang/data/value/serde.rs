//! Serde traverses syntax; only interned handles need arena access

use ::serde::{Deserializer, Serializer};
use serde_state::{DeserializeState, SerializeState};
use thiserror::Error;

use super::{Interned, Value, ValueArena, ValueError, get};
use crate::util::json::json;

/// Connects an interned type to its storage in an arena
pub trait ArenaStore<T> {
    fn lookup(&self, id: Interned<T>) -> &T;
    fn intern(&mut self, data: T) -> Result<Interned<T>, ValueError>;
}

impl<T> SerializeState<ValueArena> for Interned<T>
where
    ValueArena: ArenaStore<T>,
    T: SerializeState<ValueArena>,
{
    fn serialize_state<S>(&self, serializer: S, arena: &ValueArena) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        arena.lookup(*self).serialize_state(serializer, arena)
    }
}

impl<'de, T> DeserializeState<'de, ValueArena> for Interned<T>
where
    ValueArena: ArenaStore<T>,
    T: DeserializeState<'de, ValueArena>,
{
    fn deserialize_state<D>(arena: &mut ValueArena, deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = T::deserialize_state(arena, deserializer)?;
        arena.intern(data).map_err(::serde::de::Error::custom)
    }
}

/// Produces the JSON payload stored by a native extern
pub fn encode<T>(arena: &ValueArena, data: &T) -> Result<json, serde_json::Error>
where
    T: SerializeState<ValueArena> + ?Sized,
{
    on_serde_stack(|| data.serialize_state(serde_json::value::Serializer, arena))
}

pub fn decode<'de, T>(arena: &mut ValueArena, json: &'de json) -> Result<T, serde_json::Error>
where
    T: DeserializeState<'de, ValueArena>,
{
    on_serde_stack(|| T::deserialize_state(arena, json))
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn decode_external<T>(arena: &mut ValueArena, value: &Value) -> Result<T, DecodeError>
where
    T: for<'de> DeserializeState<'de, ValueArena>,
{
    // Retain the existing payload while restoring handles into the same arena
    let json = get::external_shared(arena, value)?.clone();
    Ok(decode(arena, json.as_ref())?)
}

fn on_serde_stack<T>(serde: impl FnOnce() -> T) -> T {
    stacker::grow(32 * 1024 * 1024, serde)
}
