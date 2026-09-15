//! V1Model parses once, runs ingress and egress, then deparses for transmission
//!
//! ```text
//! Rx -> Parser -> Verify checksum -> Ingress -> Egress
//!                                                |
//!                                                v
//! Tx <- Deparser <- Update checksum <-------------+
//! ```
//!
//! The scheduler runs queued ingress and egress packets, including clones and
//! multicast copies; dropping a packet ends its path
//!
//! Resubmit returns the original packet to the parser; recirculate returns the
//! deparsed packet to the parser

use super::super::{
    core::{
        func as core_func,
        object::{PacketIn, PacketOut, packet as core_packet},
    },
    externs as external,
    io::{Rx, Tx},
    spec_impl::{func, pack, pgm, rel, unpack},
    state::SimState,
};
use super::{
    arch::Arch,
    func as v1model_func,
    object::{Counter, DirectCounter, DirectMeter, Register},
    packet::{CloneInfo, CloneType, Entrypoint, Packet},
};
use crate::lang::data::value::external::{
    DecodeContext, EncodeContext, Encoding, decode_with, encode_with,
};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, ValueError, get, make},
        },
    },
    runner::{ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
};
use num_traits::ToPrimitive;
use serde_derive_state::{DeserializeState, SerializeState};

// == Configuration

#[derive(Default)]
pub struct V1Model {
    encoding: Encoding,
}

impl V1Model {
    pub fn new(encoding: Encoding) -> Self {
        Self { encoding }
    }
}

// == Extern objects

/// Core and v1model-specific extern objects
#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
pub enum ObjectState {
    PacketIn(PacketIn),
    PacketOut(PacketOut),
    Counter(Counter),
    Register(#[serde(state)] Register),
    DirectCounter(DirectCounter),
    DirectMeter(DirectMeter),
}

impl ObjectState {
    // - Encoding

    pub fn to_value(
        &self,
        arena: &mut ValueArena,
        encoding: Encoding,
    ) -> Result<Value, ExternError> {
        let payload = encode_with(arena, encoding, self)
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        external::state_value(arena, "objectState", payload.into())
    }

    // - Decoding

    pub fn from_value(
        arena: &mut ValueArena,
        encoding: Encoding,
        value: &Value,
    ) -> Result<Self, ExternError> {
        let json = get::external(arena, value)?.clone();
        decode_with(arena, encoding, json.as_ref())
            .map_err(|error| ExternError::Failure(error.to_string()))
    }
}

// == STF transformation

pub fn transform_stf_stmt(mut stmt: Statement) -> Statement {
    fn transform_name(name: crate::stf::ast::Name) -> crate::stf::ast::Name {
        name.rewrite_substring(&["ingress", "preqos"], "main.ig")
            .rewrite_substring(&["egress", "postqos", "c3"], "main.eg")
    }
    match &mut stmt {
        Statement::Add {
            table,
            matches,
            action,
            ..
        } => {
            *table = transform_name(table.clone());
            action.name = transform_name(action.name.clone());
            for mtch in matches {
                *mtch = mtch.clone().rewrite_valid();
            }
        }
        Statement::SetDefault { table, action } => {
            *table = transform_name(table.clone());
            action.name = transform_name(action.name.clone());
        }
        _ => {}
    }
    stmt
}

// == Architectural state

pub(super) fn init_arch_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    Arch::default()
        .to_value(ctx.arena_mut(), encoding)
        .map_err(Into::into)
}

pub fn find_arch_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
) -> Result<Arch, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    let value_state = func::find_arch_state_e(ctx, value_arch)?;
    Ok(Arch::from_value(ctx.arena_mut(), encoding, &value_state)?)
}

pub fn update_arch_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
    arch: &Arch,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    let value_state = arch.to_value(ctx.arena_mut(), encoding)?;
    func::update_arch_state_e(ctx, value_arch, value_state)
}

// == Object state

pub fn find_object_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
    value_id: Value,
) -> Result<ObjectState, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    let value_object = func::find_object_state_e(ctx, value_arch, value_id)?;
    Ok(ObjectState::from_value(
        ctx.arena_mut(),
        encoding,
        &value_object,
    )?)
}

fn find_packet_in<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
) -> Result<PacketIn, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_name = make::text(ctx.arena_mut(), "packet_in".to_owned(), Span::default())
        .map_err(ExternError::from)?;
    let values_name = vec![value_name];
    let typ_id = typ::make::list(typ::make::var(
        crate::phrase!(node: "id".to_owned(), span: Span::default()),
        vec![],
    ));
    let value_id = make::list(
        ctx.arena_mut(),
        typ_id.node.into(),
        values_name,
        Span::default(),
    )
    .map_err(ExternError::from)?;
    match find_object_state(ctx, value_arch, value_id)? {
        ObjectState::PacketIn(pkt) => Ok(pkt),
        _ => Err(ExternError::Failure("packet_in extern not found".to_owned()).into()),
    }
}

fn find_packet_out<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
) -> Result<PacketOut, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_name = make::text(ctx.arena_mut(), "packet_out".to_owned(), Span::default())
        .map_err(ExternError::from)?;
    let values_name = vec![value_name];
    let typ_id = typ::make::list(typ::make::var(
        crate::phrase!(node: "id".to_owned(), span: Span::default()),
        vec![],
    ));
    let value_id = make::list(
        ctx.arena_mut(),
        typ_id.node.into(),
        values_name,
        Span::default(),
    )
    .map_err(ExternError::from)?;
    match find_object_state(ctx, value_arch, value_id)? {
        ObjectState::PacketOut(pkt) => Ok(pkt),
        _ => Err(ExternError::Failure("packet_out extern not found".to_owned()).into()),
    }
}

// == Extern calls

// - Initialization

pub(super) fn eval_extern_init<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    values: &[Value],
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    let (value_name, value_targs, value_ids, value_args) =
        get::four(values).map_err(ExternError::from)?;
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let object = match name.as_str() {
        "counter" => Some(ObjectState::Counter(Counter::init(
            ctx.arena(),
            *value_targs,
            *value_ids,
            *value_args,
        )?)),
        "register" => Some(ObjectState::Register(Register::init(
            ctx,
            *value_targs,
            *value_ids,
            *value_args,
        )?)),
        "direct_counter" => Some(ObjectState::DirectCounter(DirectCounter::init(
            ctx.arena(),
            *value_targs,
            *value_ids,
            *value_args,
        )?)),
        "direct_meter" => Some(ObjectState::DirectMeter(DirectMeter::init(
            ctx.arena(),
            *value_targs,
            *value_ids,
            *value_args,
        )?)),
        _ => None,
    };
    Ok(match object {
        Some(object) => object.to_value(ctx.arena_mut(), encoding)?,
        None => {
            let payload = encode_with(ctx.arena(), encoding, &())
                .map_err(|error| ExternError::Failure(error.to_string()))?;
            external::state_value(ctx.arena_mut(), "objectState", payload.into())?
        }
    })
}

// - Function calls

pub(super) fn eval_extern_func_call<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch, value_name, value_names) =
        get::four(values).map_err(ExternError::from)?;
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let names = external::param_names(ctx.arena(), *value_names)?;
    let names_ref: Vec<_> = names.iter().map(String::as_str).collect();
    let (value_ctx, value_arch, value_call_result) = match (name.as_str(), names_ref.as_slice()) {
        ("verify", ["check", "toSignal"]) => core_func::verify(ctx, *value_ctx, *value_arch)?,
        ("digest", ["receiver", "data"]) => v1model_func::digest(ctx, *value_ctx, *value_arch)?,
        ("mark_to_drop", ["standard_metadata"]) => {
            v1model_func::mark_to_drop(ctx, *value_ctx, *value_arch)?
        }
        ("verify_checksum", ["condition", "data", "checksum", "algo"]) => {
            v1model_func::verify_checksum(ctx, *value_ctx, *value_arch)?
        }
        ("update_checksum", ["condition", "data", "checksum", "algo"]) => {
            v1model_func::update_checksum(ctx, *value_ctx, *value_arch)?
        }
        ("clone_preserving_field_list", ["type", "session", "index"]) => {
            v1model_func::clone_preserving_field_list(ctx, *value_ctx, *value_arch)?
        }
        ("resubmit_preserving_field_list", ["index"]) => {
            v1model_func::resubmit_preserving_field_list(ctx, *value_ctx, *value_arch)?
        }
        ("recirculate_preserving_field_list", ["index"]) => {
            v1model_func::recirculate_preserving_field_list(ctx, *value_ctx, *value_arch)?
        }
        ("hash", ["result", "algo", "base", "data", "max"]) => {
            v1model_func::hash(ctx, *value_ctx, *value_arch)?
        }
        ("log_msg", ["msg"]) => v1model_func::log_msg(ctx, *value_ctx, *value_arch)?,
        ("log_msg", ["msg", "data"]) => v1model_func::log_msg_format(ctx, *value_ctx, *value_arch)?,
        ("verify_checksum_with_payload", ["condition", "data", "checksum", "algo"]) => {
            let pkt = find_packet_in(ctx, *value_arch)?;
            v1model_func::verify_checksum_with_payload(ctx, *value_ctx, *value_arch, &pkt)?
        }
        ("update_checksum_with_payload", ["condition", "data", "checksum", "algo"]) => {
            let pkt = find_packet_in(ctx, *value_arch)?;
            v1model_func::update_checksum_with_payload(ctx, *value_ctx, *value_arch, &pkt)?
        }
        _ => {
            return Err(ExternError::Failure(format!(
                "unsupported extern function call: {name}({})",
                names.join(", ")
            ))
            .into());
        }
    };
    Ok(vec![value_ctx, value_arch, value_call_result])
}

// - Method calls

pub(super) fn eval_extern_method_call<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    let [value_ctx, value_arch, value_id, value_name, value_names] = values else {
        return Err(ExternError::Failure(
            "unexpected number of arguments to extern method call".to_owned(),
        )
        .into());
    };
    let object = find_object_state(ctx, *value_arch, *value_id)?;
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let names = external::param_names(ctx.arena(), *value_names)?;
    let names_ref: Vec<_> = names.iter().map(String::as_str).collect();
    let (object, value_ctx, value_arch, value_call_result) =
        match (object, name.as_str(), names_ref.as_slice()) {
            (ObjectState::PacketIn(object), "extract", ["hdr"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.extract(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::PacketIn(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (
                ObjectState::PacketIn(object),
                "extract",
                ["variableSizeHeader", "variableFieldSizeInBits"],
            ) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.extract_varsize(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::PacketIn(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::PacketIn(object), "lookahead", []) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.lookahead(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::PacketIn(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::PacketIn(object), "advance", ["sizeInBits"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.advance(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::PacketIn(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::PacketIn(object), "length", []) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.length(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::PacketIn(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::PacketOut(object), "emit", ["hdr"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.emit(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::PacketOut(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Counter(object), "count", ["index"]) => {
                let pkt = find_packet_in(ctx, *value_arch)?;
                let (object, value_ctx, value_arch, value_call_result) =
                    object.count(ctx, *value_ctx, *value_arch, &pkt)?;
                (
                    ObjectState::Counter(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Register(object), "read", ["result", "index"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.read(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Register(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Register(object), "write", ["index", "value"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.write(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Register(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::DirectCounter(object), "count", []) => {
                let pkt = find_packet_in(ctx, *value_arch)?;
                let (object, value_ctx, value_arch, value_call_result) =
                    object.count(ctx, *value_ctx, *value_arch, &pkt)?;
                (
                    ObjectState::DirectCounter(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::DirectMeter(object), "read", ["result"]) => {
                let pkt = find_packet_in(ctx, *value_arch)?;
                let (object, value_ctx, value_arch, value_call_result) =
                    object.read(ctx, *value_ctx, *value_arch, &pkt)?;
                (
                    ObjectState::DirectMeter(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            _ => {
                let ids = external::param_names(ctx.arena(), *value_id)?;
                return Err(ExternError::Failure(format!(
                    "unsupported extern method call: {}.{name}({})",
                    ids.join("."),
                    names.join(", ")
                ))
                .into());
            }
        };
    let value_object = object.to_value(ctx.arena_mut(), encoding)?;
    let value_arch = func::update_object_state_e(ctx, value_arch, *value_id, value_object)?;
    Ok(vec![value_ctx, value_arch, value_call_result])
}

// == Mirror table interface

pub fn add_mirror_session<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
    session: i64,
    port: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.mirrortable.insert(session, port);
    update_arch_state(ctx, value_arch, &arch)
}

pub fn add_mirror_session_mc<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    _value_arch: Value,
    _session: i64,
    _group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "add_mirror_session_mc is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

// == Multicast interface

pub fn mc_mgrp_create<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
    group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.multicast.group_create(group);
    update_arch_state(ctx, value_arch, &arch)
}

pub fn mc_node_create<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
    instance: i64,
    ports: &[i64],
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.multicast.node_create(instance, ports);
    update_arch_state(ctx, value_arch, &arch)
}

pub fn mc_node_associate<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    value_arch: Value,
    group: i64,
    handle: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.multicast.node_associate(group, handle);
    update_arch_state(ctx, value_arch, &arch)
}

// == Register interface

pub fn register_read<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    _value_arch: Value,
    _name: &str,
    _idx: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "register_read is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

pub fn register_write<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    _value_arch: Value,
    _name: &str,
    _idx: i64,
    _int: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "register_write is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

pub fn register_reset<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    _value_arch: Value,
    _name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "register_reset is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

// == Packet state

fn insert_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    packet: Packet,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    state.value_arch = {
        let value_name = make::text(ctx.arena_mut(), "packet_in".to_owned(), Span::default())
            .map_err(ExternError::from)?;
        let values_name = vec![value_name];
        let typ_id = typ::make::list(typ::make::var(
            crate::phrase!(node: "id".to_owned(), span: Span::default()),
            vec![],
        ));
        let value_id = make::list(
            ctx.arena_mut(),
            typ_id.node.into(),
            values_name,
            Span::default(),
        )
        .map_err(ExternError::from)?;
        let encoding = ctx.external().encoding;
        let value_object =
            ObjectState::PacketIn(packet.packet_in).to_value(ctx.arena_mut(), encoding)?;
        func::update_object_state_e(ctx, state.value_arch, value_id, value_object)
    }?;
    state.value_ctx = packet.value_ctx;
    Ok(())
}

fn remove_packet_in<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let mut pkt = find_packet_in(ctx, state.value_arch)?;
    pkt.reset();
    state.value_arch = {
        let value_name = make::text(ctx.arena_mut(), "packet_in".to_owned(), Span::default())
            .map_err(ExternError::from)?;
        let values_name = vec![value_name];
        let typ_id = typ::make::list(typ::make::var(
            crate::phrase!(node: "id".to_owned(), span: Span::default()),
            vec![],
        ));
        let value_id = make::list(
            ctx.arena_mut(),
            typ_id.node.into(),
            values_name,
            Span::default(),
        )
        .map_err(ExternError::from)?;
        let encoding = ctx.external().encoding;
        let value_object = ObjectState::PacketIn(pkt).to_value(ctx.arena_mut(), encoding)?;
        func::update_object_state_e(ctx, state.value_arch, value_id, value_object)
    }?;
    Ok(())
}

fn remove_packet_out<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    state.value_arch = {
        let value_name = make::text(ctx.arena_mut(), "packet_out".to_owned(), Span::default())
            .map_err(ExternError::from)?;
        let values_name = vec![value_name];
        let typ_id = typ::make::list(typ::make::var(
            crate::phrase!(node: "id".to_owned(), span: Span::default()),
            vec![],
        ));
        let value_id = make::list(
            ctx.arena_mut(),
            typ_id.node.into(),
            values_name,
            Span::default(),
        )
        .map_err(ExternError::from)?;
        let encoding = ctx.external().encoding;
        let value_object =
            ObjectState::PacketOut(PacketOut::default()).to_value(ctx.arena_mut(), encoding)?;
        func::update_object_state_e(ctx, state.value_arch, value_id, value_object)
    }?;
    Ok(())
}

fn is_dropped<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_spec",
    )?;
    let (width, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
    Ok(width == 9.into() && int == 511.into())
}

fn get_mcast_grp<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<i64, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let group = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "standard_metadata",
            "mcast_grp",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        int.to_i64()
            .ok_or_else(|| ExternError::Failure("integer outside i64 range".to_owned()))
    }?;
    Ok(group)
}

// == Pipeline initializer

pub fn init_pipe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    program: Value,
) -> Result<SimState, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch) = pgm::v1model_init(ctx, program)?;
    Ok(SimState {
        value_ctx,
        value_arch,
        txs: vec![],
    })
}

// == Pipeline driver

pub fn setup_rx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    rx: &Rx,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let encoding = ctx.external().encoding;
    // Setup packet input, output and global variables in source order
    let value_packet =
        ObjectState::PacketIn(PacketIn::init(&rx.packet)?).to_value(ctx.arena_mut(), encoding)?;
    let (value_ctx, value_arch) =
        rel::v1model_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    let value_packet =
        ObjectState::PacketOut(PacketOut::default()).to_value(ctx.arena_mut(), encoding)?;
    let (value_ctx, value_arch) =
        rel::v1model_init_packet_out(ctx, state.value_ctx, state.value_arch, value_packet)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    state.value_ctx = rel::v1model_init_globals(ctx, state.value_ctx, state.value_arch, rx.port)?;
    Ok(())
}

// == Parser + Verify

pub fn drive_p<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch, value_call_result) =
        rel::v1model_parser(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    let value_error = get::matches! { ctx.arena(), &value_call_result,
        "REJECT errorValue" => |values| match values.as_slice() {
            [value_error] => Some(**value_error),
            _ => return Err(ExternError::from(ValueError::ExpectedCount {
                expected: 1,
                actual: values.len(),
            }).into()),
        },
        _ => None,
    };
    if let Some(value_error) = value_error {
        state.value_ctx = rel::lvalue_write_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "standard_metadata",
            "parser_error",
            value_error,
        )?;
    }
    Ok(())
}

pub fn drive_vr<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch, value_result) =
        rel::v1model_verify(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

pub fn drive_pipe_pre<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let mut arch = find_arch_state(ctx, state.value_arch)?;
    arch.reset();
    state.value_arch = update_arch_state(ctx, state.value_arch, &arch)?;
    remove_packet_in(ctx, state)?;
    drive_p(ctx, state)?;
    drive_vr(ctx, state)
}

// == Checksum + Deparser

pub fn drive_ck<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch, value_result) =
        rel::v1model_check(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

pub fn drive_dep<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch, value_result) =
        rel::v1model_deparse(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

pub fn drive_pipe_post<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    drive_ck(ctx, state)?;
    remove_packet_out(ctx, state)?;
    drive_dep(ctx, state)?;
    let port = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "standard_metadata",
            "egress_spec",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        int.to_i64()
            .ok_or_else(|| ExternError::Failure("integer outside i64 range".to_owned()))
    }?;
    let packet = {
        let pkt_in = find_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    state.txs.push(Tx { port, packet });
    Ok(())
}

// == Prepare context for resubmit/clone/recirculate/multicast

fn prepare_resubmit_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    idx: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_idx = pack::p4_fixed_bit(ctx.arena_mut(), 8.into(), idx.into())?;
    state.value_ctx = rel::v1model_setup_preserved_meta_fields(
        ctx,
        state.value_ctx,
        state.value_arch,
        value_idx,
    )?;
    // PKT_INSTANCE_TYPE_RESUBMIT
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 32.into(), 6.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "instance_type",
        value,
    )?;
    Ok(())
}

fn prepare_clone_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    clone_type: CloneType,
    port: i64,
    idx: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_idx = pack::p4_fixed_bit(ctx.arena_mut(), 8.into(), idx.into())?;
    state.value_ctx = rel::v1model_setup_preserved_meta_fields(
        ctx,
        state.value_ctx,
        state.value_arch,
        value_idx,
    )?;
    let instance = match clone_type {
        CloneType::I2E => 1,
        CloneType::E2E => 2,
    };
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 32.into(), instance.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "instance_type",
        value,
    )?;
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 9.into(), port.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_spec",
        value,
    )?;
    Ok(())
}

fn prepare_recirculate_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    idx: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_idx = pack::p4_fixed_bit(ctx.arena_mut(), 8.into(), idx.into())?;
    state.value_ctx = rel::v1model_setup_preserved_meta_fields(
        ctx,
        state.value_ctx,
        state.value_arch,
        value_idx,
    )?;
    // PKT_INSTANCE_TYPE_RECIRC
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 32.into(), 4.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "instance_type",
        value,
    )?;
    Ok(())
}

fn prepare_multicast_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    rid: i64,
    port: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 16.into(), rid.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_rid",
        value,
    )?;
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 9.into(), port.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_spec",
        value,
    )?;
    // PKT_INSTANCE_TYPE_REPLICATION
    let value = pack::p4_fixed_bit(ctx.arena_mut(), 32.into(), 5.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "instance_type",
        value,
    )?;
    Ok(())
}

// == Schedule resubmit/clone/recirculate/multicast if needed

pub fn schedule_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    entrypoint: Entrypoint,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let packet_in = find_packet_in(ctx, state.value_arch)?;
    let packet = Packet {
        value_ctx: state.value_ctx,
        packet_in,
        entrypoint,
    };
    let mut arch = find_arch_state(ctx, state.value_arch)?;
    match entrypoint {
        Entrypoint::Ingress => arch.queue.push_front(packet),
        Entrypoint::Egress => arch.queue.push_back(packet),
    }
    state.value_arch = update_arch_state(ctx, state.value_arch, &arch)?;
    Ok(())
}

pub fn schedule_resubmit<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    arch: &Arch,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let Some(idx) = arch.action.resubmit_opt else {
        return Ok(false);
    };
    let value_ctx_original = state.value_ctx;
    prepare_resubmit_ctx(ctx, state, idx)?;
    drive_pipe_pre(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    state.value_ctx = value_ctx_original;
    Ok(true)
}

pub fn schedule_clone<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    arch: &Arch,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let Some(CloneInfo(clone_type, session, idx)) = arch.action.clone_opt else {
        return Ok(false);
    };
    let Some(&port) = arch.mirrortable.get(&session) else {
        return Ok(false);
    };
    let value_ctx_original = state.value_ctx;
    prepare_clone_ctx(ctx, state, clone_type, port, idx)?;
    if clone_type == CloneType::I2E {
        drive_pipe_pre(ctx, state)?;
    }
    schedule_packet(ctx, state, Entrypoint::Egress)?;
    state.value_ctx = value_ctx_original;
    Ok(true)
}

pub fn schedule_recirculate<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    arch: &Arch,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let Some(idx) = arch.action.recirculate_opt else {
        return Ok(false);
    };
    let value_ctx_original = state.value_ctx;
    prepare_recirculate_ctx(ctx, state, idx)?;
    // Run checksum and deparser before feeding the output back to the parser
    drive_ck(ctx, state)?;
    remove_packet_out(ctx, state)?;
    drive_dep(ctx, state)?;
    let packet = {
        let pkt_in = find_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = {
        let value_name = make::text(ctx.arena_mut(), "packet_in".to_owned(), Span::default())
            .map_err(ExternError::from)?;
        let values_name = vec![value_name];
        let typ_id = typ::make::list(typ::make::var(
            crate::phrase!(node: "id".to_owned(), span: Span::default()),
            vec![],
        ));
        let value_id = make::list(
            ctx.arena_mut(),
            typ_id.node.into(),
            values_name,
            Span::default(),
        )
        .map_err(ExternError::from)?;
        let encoding = ctx.external().encoding;
        let value_object = pkt.to_value(ctx.arena_mut(), encoding)?;
        func::update_object_state_e(ctx, state.value_arch, value_id, value_object)
    }?;
    // Parser and verify run before the ingress packet enters the queue
    drive_pipe_pre(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    state.value_ctx = value_ctx_original;
    Ok(true)
}

pub fn schedule_multicast<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    arch: &Arch,
    group: i64,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let Some(handles) = arch.multicast.groups.get(&group) else {
        return Ok(false);
    };
    for handle in handles {
        if let Some(nodes) = arch.multicast.nodes.get(handle) {
            for node in nodes {
                prepare_multicast_ctx(ctx, state, node.rid, node.port)?;
                schedule_packet(ctx, state, Entrypoint::Egress)?;
            }
        }
    }
    Ok(true)
}

// == Ingress + Handle clone, resubmit, drop

pub fn drive_ig<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let (value_ctx, value_arch, value_result) =
        rel::v1model_ingress(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    let arch = find_arch_state(ctx, state.value_arch)?;
    schedule_clone(ctx, state, &arch)?;
    if schedule_resubmit(ctx, state, &arch)? {
        return Ok(value_result);
    }
    let group = get_mcast_grp(ctx, state)?;
    if group != 0 {
        schedule_multicast(ctx, state, &arch, group)?;
    } else if !is_dropped(ctx, state)? {
        schedule_packet(ctx, state, Entrypoint::Egress)?;
    }
    Ok(value_result)
}

fn prepare_egress_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let value_port = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_spec",
    )?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_port",
        value_port,
    )?;
    Ok(())
}

pub fn drive_eg<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<Option<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    prepare_egress_ctx(ctx, state)?;
    let (value_ctx, value_arch, value_result) =
        rel::v1model_egress(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    let arch = find_arch_state(ctx, state.value_arch)?;
    schedule_clone(ctx, state, &arch)?;
    if is_dropped(ctx, state)? {
        return Ok(None);
    }
    let arch = find_arch_state(ctx, state.value_arch)?;
    if schedule_recirculate(ctx, state, &arch)? {
        Ok(None)
    } else {
        Ok(Some(value_result))
    }
}

// == Scheduling packets

pub fn drive_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    packet: Packet,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let entrypoint = packet.entrypoint;
    insert_packet(ctx, state, packet)?;
    match entrypoint {
        Entrypoint::Ingress => {
            drive_ig(ctx, state)?;
            Ok(())
        }
        Entrypoint::Egress => match drive_eg(ctx, state)? {
            Some(_) => drive_pipe_post(ctx, state),
            None => Ok(()),
        },
    }
}

pub fn run_scheduler<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    loop {
        let mut arch = find_arch_state(ctx, state.value_arch)?;
        let Some(packet) = arch.queue.pop_front() else {
            return Ok(());
        };
        arch.reset();
        state.value_arch = update_arch_state(ctx, state.value_arch, &arch)?;
        drive_packet(ctx, state, packet)?;
    }
}

pub fn drive_pipe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    state: &mut SimState,
    rx: &Rx,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    state.txs.clear();
    setup_rx(ctx, state, rx)?;
    drive_pipe_pre(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    run_scheduler(ctx, state)
}
