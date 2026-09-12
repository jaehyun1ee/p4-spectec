use super::super::externs;
use super::{
    mirror, multicast,
    packet::{Packet, PacketJson},
};
use crate::{
    lang::data::value::{Value, ValueArena, get},
    runner::ExternError,
    util::json::json,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Architectural state with an empty-state default constructor
pub struct Arch {
    pub queue: VecDeque<Packet>,
    pub mirrortable: mirror::Table,
    pub multicast: multicast::State,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchJson {
    queue: VecDeque<PacketJson>,
    mirrortable: mirror::Table,
    multicast: multicast::State,
}

impl Arch {
    pub fn to_json(&self, arena: &ValueArena) -> Result<json, ExternError> {
        let queue = self
            .queue
            .iter()
            .map(|pkt| PacketJson::encode(arena, pkt))
            .collect::<Result<_, _>>()?;
        serde_json::to_value(ArchJson {
            queue,
            mirrortable: self.mirrortable.clone(),
            multicast: self.multicast.clone(),
        })
        .map_err(|error| ExternError::Failure(error.to_string()))
    }

    pub fn from_json(arena: &mut ValueArena, json: &json) -> Result<Self, ExternError> {
        let arch: ArchJson = serde_json::from_value(json.clone())
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        let queue = arch
            .queue
            .into_iter()
            .map(|pkt| pkt.decode(arena))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            queue,
            mirrortable: arch.mirrortable,
            multicast: arch.multicast,
        })
    }

    /// Value conversion
    pub fn to_value(&self, arena: &mut ValueArena) -> Result<Value, ExternError> {
        let json = self.to_json(arena)?;
        externs::state_value(arena, "archState", json)
    }

    pub fn from_value(arena: &mut ValueArena, value: &Value) -> Result<Self, ExternError> {
        let json = get::external(arena, value)?.clone();
        Self::from_json(arena, &json)
    }
}
