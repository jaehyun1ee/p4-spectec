//! Packet clone requests, pending actions and processing contexts
//! For example, an I2E clone records a mirror session and a field-list index

use serde::{Deserialize, Serialize};
use serde_derive_state::{DeserializeState, SerializeState};

use crate::{
    lang::data::value::{
        Value, ValueArena,
        external::{DecodeContext, EncodeContext},
    },
    runner::ExternError,
};

use super::super::{core::object::PacketIn, spec::unpack};

// == Packet clones

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloneType {
    I2E,
    E2E,
}

/// Packet clone direction, mirror session and preserved field-list index
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneInfo(pub CloneType, pub usize, pub usize);

impl CloneInfo {
    pub fn new(
        arena: &ValueArena,
        value_clone_type: &Value,
        value_session: &Value,
        value_idx: &Value,
    ) -> Result<Self, ExternError> {
        let (_, name) = unpack::p4_enum(arena, value_clone_type)?;
        let clone_type = match name.as_str() {
            "I2E" => CloneType::I2E,
            "E2E" => CloneType::E2E,
            name => {
                return Err(ExternError::Failure(format!(
                    "Invalid enum value \"{name}\". Expected I2E or E2E"
                )));
            }
        };
        let session = usize::try_from(&unpack::p4_fixed_bit(arena, value_session)?.1)?;
        let idx = usize::try_from(&unpack::p4_fixed_bit(arena, value_idx)?.1)?;
        Ok(Self(clone_type, session, idx))
    }
}

// == Actions on a packet

/// Actions requested by extern calls for the current packet
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub clone_opt: Option<CloneInfo>,
    pub resubmit_opt: Option<usize>,
    pub recirculate_opt: Option<usize>,
}

// == Processing context per packet

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entrypoint {
    Ingress,
    Egress,
}

/// Processing context per packet
#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(deny_unknown_fields, serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
pub struct Packet {
    /// Evaluation context
    #[serde(state)]
    pub value_ctx: Value,
    /// Packet input
    pub packet_in: PacketIn,
    /// Block to resume after parser and verify have already run
    pub entrypoint: Entrypoint,
}
