use super::super::core::object::PacketIn;
use crate::lang::data::value::Value;
use crate::lang::data::value::external::{DecodeContext, EncodeContext};
use serde::{Deserialize, Serialize};
use serde_derive_state::{DeserializeState, SerializeState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entrypoint {
    Ingress,
    Egress,
}

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(
    deny_unknown_fields,
    serialize_state = "EncodeContext<'arena>",
    ser_parameters = "'arena"
)]
#[serde(deserialize_state = "DecodeContext<'de>")]
/// Processing context per packet
pub struct Packet {
    /// Evaluation context
    #[serde(state)]
    pub value_ctx: Value,
    /// Packet input
    #[serde(
        serialize_with = "PacketIn::serialize_validated",
        deserialize_with = "PacketIn::deserialize_validated"
    )]
    pub packet_in: PacketIn,
    /// Which pipeline the packet should begin processing
    pub entrypoint: Entrypoint,
}
