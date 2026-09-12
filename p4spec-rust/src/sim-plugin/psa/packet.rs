use super::super::core::object::PacketIn;
use crate::{
    lang::data::{
        serialize::value,
        value::{Value, ValueArena},
    },
    runner::ExternError,
    util::json::json,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entrypoint {
    Ingress,
    Egress,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Processing context per packet
pub struct Packet {
    /// Evaluation context
    pub value_ctx: Value,
    /// Packet input
    pub packet_in: PacketIn,
    /// Which pipeline the packet should begin processing
    pub entrypoint: Entrypoint,
}

// The queued context must encode its value tree, rather than an arena handle
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PacketJson {
    json_ctx: json,
    packet_in: PacketIn,
    entrypoint: Entrypoint,
}

impl PacketJson {
    pub(super) fn encode(arena: &ValueArena, pkt: &Packet) -> Result<Self, ExternError> {
        pkt.packet_in.payload()?;
        Ok(Self {
            json_ctx: value::encode(arena, &pkt.value_ctx),
            packet_in: pkt.packet_in.clone(),
            entrypoint: pkt.entrypoint,
        })
    }

    pub(super) fn decode(self, arena: &mut ValueArena) -> Result<Packet, ExternError> {
        self.packet_in.payload()?;
        let value_ctx = value::decode(arena, &self.json_ctx).map_err(ExternError::from)?;
        Ok(Packet {
            value_ctx,
            packet_in: self.packet_in,
            entrypoint: self.entrypoint,
        })
    }
}

impl Packet {
    pub fn to_json(&self, arena: &ValueArena) -> Result<json, ExternError> {
        serde_json::to_value(PacketJson::encode(arena, self)?)
            .map_err(|error| ExternError::Failure(error.to_string()))
    }

    pub fn from_json(arena: &mut ValueArena, json: &json) -> Result<Self, ExternError> {
        let pkt: PacketJson = serde_json::from_value(json.clone())
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        pkt.decode(arena)
    }
}
