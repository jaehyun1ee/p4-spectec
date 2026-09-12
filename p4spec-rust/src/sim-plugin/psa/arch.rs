use super::super::externs;
use super::{mirror, multicast, packet::Packet};
use crate::{
    lang::data::value::{Value, ValueArena, serde},
    runner::ExternError,
};
use serde_derive_state::{DeserializeState, SerializeState};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(deny_unknown_fields, serialize_state = "ValueArena")]
#[serde(deserialize_state = "ValueArena")]
/// Architectural state with an empty-state default constructor
pub struct Arch {
    #[serde(state)]
    pub queue: VecDeque<Packet>,
    pub mirrortable: mirror::Table,
    pub multicast: multicast::State,
}

impl Arch {
    /// Value conversion
    pub fn to_value(&self, arena: &mut ValueArena) -> Result<Value, ExternError> {
        let json =
            serde::encode(arena, self).map_err(|error| ExternError::Failure(error.to_string()))?;
        externs::state_value(arena, "archState", json)
    }

    pub fn from_value(arena: &mut ValueArena, value: &Value) -> Result<Self, ExternError> {
        serde::decode_external(arena, value).map_err(ExternError::from)
    }
}
