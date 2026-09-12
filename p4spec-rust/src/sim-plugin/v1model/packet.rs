use super::super::core::object::PacketIn;
use crate::lang::data::value::{Value, ValueArena};
use serde::{Deserialize, Serialize};
use serde_derive_state::{DeserializeState, SerializeState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entrypoint {
    Ingress,
    Egress,
}

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(deny_unknown_fields, serialize_state = "ValueArena")]
#[serde(deserialize_state = "ValueArena")]
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
    /// Block to resume after parser and verify have already run
    pub entrypoint: Entrypoint,
}

/// Packet clone direction, mirror session and preserved field-list index
pub type CloneInfo = (CloneType, i64, i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloneType {
    I2E,
    E2E,
}

/// Actions requested by extern calls for the current packet
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub clone_opt: Option<CloneInfo>,
    pub resubmit_opt: Option<i64>,
    pub recirculate_opt: Option<i64>,
}

pub fn clone_info(
    arena: &ValueArena,
    value_type: &Value,
    value_session: &Value,
    value_index: &Value,
) -> Result<CloneInfo, crate::runner::ExternError> {
    use crate::sim_plugin::spec_impl::unpack;
    let (_, name) = unpack::p4_enum(arena, value_type)?;
    let clone_type = match name.as_str() {
        "I2E" => CloneType::I2E,
        "E2E" => CloneType::E2E,
        name => {
            return Err(crate::runner::ExternError::Failure(format!(
                "Invalid enum value \"{name}\". Expected I2E or E2E"
            )));
        }
    };
    Ok((
        clone_type,
        field_index(arena, value_session)?,
        field_index(arena, value_index)?,
    ))
}

pub fn field_index(arena: &ValueArena, value: &Value) -> Result<i64, crate::runner::ExternError> {
    use crate::sim_plugin::spec_impl::unpack;
    unpack::signed_int(&unpack::p4_fixed_bit(arena, value)?.int)
}
