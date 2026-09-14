use super::super::externs;
use super::{
    mirror, multicast,
    packet::{Action, Packet},
};
use crate::lang::data::value::external::{
    DecodeContext, EncodeContext, Encoding, decode_with, encode_with,
};
use crate::{
    lang::data::value::{Value, ValueArena, get},
    runner::ExternError,
};
use serde_derive_state::{DeserializeState, SerializeState};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(
    deny_unknown_fields,
    serialize_state = "EncodeContext<'arena>",
    ser_parameters = "'arena"
)]
#[serde(deserialize_state = "DecodeContext<'de>")]
/// Architectural state with an empty-state default constructor
pub struct Arch {
    #[serde(state)]
    pub queue: VecDeque<Packet>,
    pub mirrortable: mirror::Table,
    pub multicast: multicast::State,
    pub action: Action,
}

impl Arch {
    /// Reset only the current packet's requested actions
    pub fn reset(&mut self) {
        self.action = Action::default();
    }

    /// Value conversion
    pub fn to_value(
        &self,
        arena: &mut ValueArena,
        encoding: Encoding,
    ) -> Result<Value, ExternError> {
        let payload = encode_with(arena, encoding, self)
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        externs::state_value(arena, "archState", payload.into())
    }

    pub fn from_value(
        arena: &mut ValueArena,
        encoding: Encoding,
        value: &Value,
    ) -> Result<Self, ExternError> {
        let json = get::external_shared(arena, value)?.clone();
        decode_with(arena, encoding, json.as_ref())
            .map_err(|error| ExternError::Failure(error.to_string()))
    }
}
