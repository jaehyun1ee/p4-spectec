use super::super::{
    core::{
        func as core_func,
        object::{self as core_object, PacketIn, PacketOut},
    },
    externs as external,
    io::Transmission,
    spec_impl::{func, pack, pgm, rel, unpack},
    state::{SimState, install_result},
};
use super::{
    arch::Arch,
    func as v1model_func,
    object::{Counter, DirectCounter, DirectMeter, Register},
    packet::{CloneType, Entrypoint, Packet},
};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make, serde},
        },
        il::ast::Typ,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
    util::json::json,
};
use serde_derive_state::{DeserializeState, SerializeState};

pub struct V1Model;

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(serialize_state = "ValueArena")]
#[serde(deserialize_state = "ValueArena")]
/// Core and v1model-specific extern objects
pub enum ObjectState {
    PacketIn(#[serde(deserialize_with = "PacketIn::deserialize_validated")] PacketIn),
    PacketOut(PacketOut),
    Counter(Counter),
    Register(#[serde(state)] Register),
    DirectCounter(DirectCounter),
    DirectMeter(DirectMeter),
}

impl ObjectState {
    pub fn from_value(arena: &mut ValueArena, value: &Value) -> Result<Self, ExternError> {
        serde::decode_external(arena, value).map_err(ExternError::from)
    }

    pub fn to_value(&self, arena: &mut ValueArena) -> Result<Value, ExternError> {
        let json =
            serde::encode(arena, self).map_err(|error| ExternError::Failure(error.to_string()))?;
        external::state_value(arena, "objectState", json)
    }
}

// Extern calls

impl Extern for V1Model {
    fn eval_func<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        _targs: &[Typ],
        values: &[Value],
    ) -> Result<(Value, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let value = match name {
            "init_archState" => Arch::default().to_value(ctx.arena_mut())?,
            "init_objectState" => {
                let [value_name, value_targs, value_ids, value_args] = values else {
                    return Err(ExternError::Failure(
                        "unexpected number of arguments to extern init".to_owned(),
                    )
                    .into());
                };
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
                match object {
                    Some(object) => object.to_value(ctx.arena_mut())?,
                    None => external::state_value(ctx.arena_mut(), "objectState", json::Null)?,
                }
            }
            _ => {
                return Err(
                    ExternError::Failure(format!("unimplemented extern function: {name}")).into(),
                );
            }
        };
        Ok((value, false))
    }
    fn eval_rel<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        values: &[Value],
    ) -> Result<(Vec<Value>, bool), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let values = match name {
            "ExternFunctionCall_eval_lctk" => external::eval_func_lctk(ctx, values)?,
            "ExternFunctionCall_eval" => eval_func(ctx, values)?,
            "ExternMethodCall_eval" => eval_method(ctx, values)?,
            _ => {
                return Err(
                    ExternError::Failure(format!("unimplemented extern relation: {name}")).into(),
                );
            }
        };
        Ok((values, false))
    }
    fn clear(&mut self) {}
}

fn eval_func<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let [value_ctx, value_arch, value_name, value_names] = values else {
        return Err(ExternError::Failure(
            "unexpected number of arguments to extern function call".to_owned(),
        )
        .into());
    };
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let names = external::param_names(ctx.arena(), *value_names)?;
    let names_ref: Vec<_> = names.iter().map(String::as_str).collect();
    let result = match (name.as_str(), names_ref.as_slice()) {
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
            let pkt = get_packet_in(ctx, *value_arch, "packet_in")?;
            v1model_func::verify_checksum_with_payload(ctx, *value_ctx, *value_arch, &pkt)?
        }
        ("update_checksum_with_payload", ["condition", "data", "checksum", "algo"]) => {
            let pkt = get_packet_in(ctx, *value_arch, "packet_in")?;
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
    Ok(vec![
        result.value_ctx,
        result.value_arch,
        result.value_call_result,
    ])
}

fn eval_method<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, V1Model>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, V1Model>,
{
    let [value_ctx, value_arch, value_id, value_name, value_names] = values else {
        return Err(ExternError::Failure(
            "unexpected number of arguments to extern method call".to_owned(),
        )
        .into());
    };
    let object = get_object_state(ctx, *value_arch, *value_id)?;
    let name = get::text(ctx.arena(), value_name)
        .map_err(ExternError::from)?
        .to_owned();
    let names = external::param_names(ctx.arena(), *value_names)?;
    let names_ref: Vec<_> = names.iter().map(String::as_str).collect();
    let (object, result) = match (object, name.as_str(), names_ref.as_slice()) {
        (ObjectState::PacketIn(object), "extract", ["hdr"]) => {
            let output = object.extract(ctx, *value_ctx, *value_arch)?;
            (ObjectState::PacketIn(output.pkt), output.result)
        }
        (
            ObjectState::PacketIn(object),
            "extract",
            ["variableSizeHeader", "variableFieldSizeInBits"],
        ) => {
            let output = object.extract_varsize(ctx, *value_ctx, *value_arch)?;
            (ObjectState::PacketIn(output.pkt), output.result)
        }
        (ObjectState::PacketIn(object), "lookahead", []) => {
            let output = object.lookahead(ctx, *value_ctx, *value_arch)?;
            (ObjectState::PacketIn(output.pkt), output.result)
        }
        (ObjectState::PacketIn(object), "advance", ["sizeInBits"]) => {
            let output = object.advance(ctx, *value_ctx, *value_arch)?;
            (ObjectState::PacketIn(output.pkt), output.result)
        }
        (ObjectState::PacketIn(object), "length", []) => {
            let output = object.length(ctx, *value_ctx, *value_arch)?;
            (ObjectState::PacketIn(output.pkt), output.result)
        }
        (ObjectState::PacketOut(object), "emit", ["hdr"]) => {
            let output = object.emit(ctx, *value_ctx, *value_arch)?;
            (ObjectState::PacketOut(output.pkt), output.result)
        }
        (ObjectState::Counter(object), "count", ["index"]) => {
            let pkt = get_packet_in(ctx, *value_arch, "packet_in")?;
            let output = object.count(ctx, *value_ctx, *value_arch, &pkt)?;
            (ObjectState::Counter(output.object), output.result)
        }
        (ObjectState::Register(object), "read", ["result", "index"]) => {
            let output = object.read(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Register(output.object), output.result)
        }
        (ObjectState::Register(object), "write", ["index", "value"]) => {
            let output = object.write(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Register(output.object), output.result)
        }
        (ObjectState::DirectCounter(object), "count", []) => {
            let pkt = get_packet_in(ctx, *value_arch, "packet_in")?;
            let output = object.count(ctx, *value_ctx, *value_arch, &pkt)?;
            (ObjectState::DirectCounter(output.object), output.result)
        }
        (ObjectState::DirectMeter(object), "read", ["result"]) => {
            let pkt = get_packet_in(ctx, *value_arch, "packet_in")?;
            let output = object.read(ctx, *value_ctx, *value_arch, &pkt)?;
            (ObjectState::DirectMeter(output.object), output.result)
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
    let value_object = object.to_value(ctx.arena_mut())?;
    let value_arch = func::update_object_state_e(ctx, result.value_arch, *value_id, value_object)?;
    Ok(vec![result.value_ctx, value_arch, result.value_call_result])
}

fn object_id(arena: &mut ValueArena, name: &str) -> Result<Value, ExternError> {
    let values = name
        .split('.')
        .map(|name| make::text(arena, name.to_owned(), Span::default()))
        .collect::<Result<Vec<_>, _>>()?;
    let typ = typ::make::list(typ::make::var(
        crate::phrase!(node: "id".to_owned(), span: Span::default()),
        vec![],
    ));
    Ok(make::list(arena, typ.node.into(), values, Span::default())?)
}

pub fn get_object_state<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    value_id: Value,
) -> Result<ObjectState, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_object = func::find_object_state_e(ctx, value_arch, value_id)?;
    Ok(ObjectState::from_value(ctx.arena_mut(), &value_object)?)
}

/// Architectural state
pub fn get_arch_state<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
) -> Result<Arch, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_state = func::find_arch_state_e(ctx, value_arch)?;
    Ok(Arch::from_value(ctx.arena_mut(), &value_state)?)
}

/// Update the queue, mirror table and multicast state in the architecture
pub fn set_arch_state<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    arch: &Arch,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_state = arch.to_value(ctx.arena_mut())?;
    func::update_arch_state_e(ctx, value_arch, value_state)
}

fn put_object<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
    object: &ObjectState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_id = object_id(ctx.arena_mut(), name)?;
    let value_object = object.to_value(ctx.arena_mut())?;
    func::update_object_state_e(ctx, value_arch, value_id, value_object)
}

fn get_packet_in<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
) -> Result<PacketIn, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_id = object_id(ctx.arena_mut(), name)?;
    match get_object_state(ctx, value_arch, value_id)? {
        ObjectState::PacketIn(pkt) => Ok(pkt),
        _ => Err(ExternError::Failure(format!("{name} extern not found")).into()),
    }
}

fn get_packet_out<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
) -> Result<PacketOut, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_id = object_id(ctx.arena_mut(), name)?;
    match get_object_state(ctx, value_arch, value_id)? {
        ObjectState::PacketOut(pkt) => Ok(pkt),
        _ => Err(ExternError::Failure(format!("{name} extern not found")).into()),
    }
}

/// Mirror session interface
pub fn add_mirror_session<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    session: i64,
    port: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut arch = get_arch_state(ctx, value_arch)?;
    arch.mirrortable.insert(session, port);
    set_arch_state(ctx, value_arch, &arch)
}

/// Multicast interface
pub fn mc_mgrp_create<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut arch = get_arch_state(ctx, value_arch)?;
    arch.multicast.group_create(group);
    set_arch_state(ctx, value_arch, &arch)
}

pub fn mc_node_create<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    instance: i64,
    ports: &[i64],
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut arch = get_arch_state(ctx, value_arch)?;
    arch.multicast.node_create(instance, ports);
    set_arch_state(ctx, value_arch, &arch)
}

pub fn mc_node_associate<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    group: i64,
    handle: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut arch = get_arch_state(ctx, value_arch)?;
    arch.multicast.node_associate(group, handle);
    set_arch_state(ctx, value_arch, &arch)
}

pub fn add_mirror_session_mc<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    _value_arch: Value,
    _session: i64,
    _group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "add_mirror_session_mc is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

pub fn register_read<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    _value_arch: Value,
    _name: &str,
    _idx: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "register_read is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

pub fn register_write<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    _value_arch: Value,
    _name: &str,
    _idx: i64,
    _int: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "register_write is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

pub fn register_reset<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    _value_arch: Value,
    _name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let _ = ctx;
    Err(ExternError::Failure(
        "register_reset is not implemented for the v1model simulator".to_owned(),
    )
    .into())
}

/// Rewrite target-specific names and header validity matches
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

/// Pipeline initializer
pub fn init_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    program: Value,
) -> Result<SimState, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = pgm::v1model_init(ctx, program)?;
    Ok(SimState {
        value_ctx: result.value_ctx,
        value_arch: result.value_arch,
        txs: vec![],
    })
}

fn insert_packet<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    packet: Packet,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        "packet_in",
        &ObjectState::PacketIn(packet.packet_in),
    )?;
    state.value_ctx = packet.value_ctx;
    Ok(())
}

fn remove_packet_in<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut pkt = get_packet_in(ctx, state.value_arch, "packet_in")?;
    pkt.reset();
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        "packet_in",
        &ObjectState::PacketIn(pkt),
    )?;
    Ok(())
}

fn remove_packet_out<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        "packet_out",
        &ObjectState::PacketOut(PacketOut::default()),
    )?;
    Ok(())
}

fn packet_string<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
) -> Result<String, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let pkt_in = get_packet_in(ctx, value_arch, "packet_in")?;
    let pkt_out = get_packet_out(ctx, value_arch, "packet_out")?;
    Ok(core_object::packet_to_string(&pkt_in, &pkt_out)?)
}

fn is_dropped<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        "egress_spec",
    )?;
    let num = unpack::p4_fixed_bit(ctx.arena(), &value)?;
    Ok(num.width == 9.into() && num.int == 511.into())
}

fn metadata_int<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &SimState,
    field: &str,
) -> Result<i64, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        field,
    )?;
    Ok(unpack::signed_int(
        &unpack::p4_fixed_bit(ctx.arena(), &value)?.int,
    )?)
}

fn write_metadata<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    field: &str,
    width: i64,
    int: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value = pack::p4_fixed_bit(ctx.arena_mut(), width.into(), int.into())?;
    state.value_ctx = rel::lvalue_write_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "standard_metadata",
        field,
        value,
    )?;
    Ok(())
}

pub fn setup_rx<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    rx: &Transmission,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    // Setup packet input, output and global variables in source order
    let value_packet =
        ObjectState::PacketIn(PacketIn::init(&rx.packet)?).to_value(ctx.arena_mut())?;
    let result = rel::v1model_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    let value_packet = ObjectState::PacketOut(PacketOut::default()).to_value(ctx.arena_mut())?;
    let result =
        rel::v1model_init_packet_out(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    state.value_ctx = rel::v1model_init_globals(ctx, state.value_ctx, state.value_arch, rx.port)?;
    Ok(())
}

/// Parser rejection records parser_error and still proceeds to verify
pub fn drive_p<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = rel::v1model_parser(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    let value_error = get::matches! { ctx.arena(), &result.value_call_result,
        "REJECT errorValue" => |values| Some(*get::one(&values.into_iter().copied().collect::<Vec<_>>()).map_err(ExternError::from)?),
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

pub fn drive_vr<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = rel::v1model_verify(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    Ok(())
}

pub fn drive_ck<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = rel::v1model_check(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    Ok(())
}

pub fn drive_dep<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = rel::v1model_deparse(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    Ok(())
}

/// Reset packet actions and cursor, then execute parser and verify
pub fn drive_pipe_pre<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut arch = get_arch_state(ctx, state.value_arch)?;
    arch.reset();
    state.value_arch = set_arch_state(ctx, state.value_arch, &arch)?;
    remove_packet_in(ctx, state)?;
    drive_p(ctx, state)?;
    drive_vr(ctx, state)
}

/// Checksum and deparser output followed by the unconsumed input payload
pub fn drive_pipe_post<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    drive_ck(ctx, state)?;
    remove_packet_out(ctx, state)?;
    drive_dep(ctx, state)?;
    let port = metadata_int(ctx, state, "egress_spec")?;
    let packet = packet_string(ctx, state.value_arch)?;
    state.txs.push(Transmission { port, packet });
    Ok(())
}

fn prepare_preserved_fields<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    index: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_index = pack::p4_fixed_bit(ctx.arena_mut(), 8.into(), index.into())?;
    state.value_ctx = rel::v1model_setup_preserved_meta_fields(
        ctx,
        state.value_ctx,
        state.value_arch,
        value_index,
    )?;
    Ok(())
}

fn prepare_resubmit_ctx<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    index: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    prepare_preserved_fields(ctx, state, index)?;
    // PKT_INSTANCE_TYPE_RESUBMIT
    write_metadata(ctx, state, "instance_type", 32, 6)
}

fn prepare_clone_ctx<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    clone_type: CloneType,
    port: i64,
    index: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    prepare_preserved_fields(ctx, state, index)?;
    let instance = match clone_type {
        CloneType::I2E => 1,
        CloneType::E2E => 2,
    };
    write_metadata(ctx, state, "instance_type", 32, instance)?;
    write_metadata(ctx, state, "egress_spec", 9, port)
}

fn prepare_recirculate_ctx<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    index: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    prepare_preserved_fields(ctx, state, index)?;
    // PKT_INSTANCE_TYPE_RECIRC
    write_metadata(ctx, state, "instance_type", 32, 4)
}

fn prepare_multicast_ctx<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    rid: i64,
    port: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    write_metadata(ctx, state, "egress_rid", 16, rid)?;
    write_metadata(ctx, state, "egress_spec", 9, port)?;
    // PKT_INSTANCE_TYPE_REPLICATION
    write_metadata(ctx, state, "instance_type", 32, 5)
}

/// Ingress reentry takes priority over queued egress packets
pub fn schedule_packet<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    entrypoint: Entrypoint,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let packet_in = get_packet_in(ctx, state.value_arch, "packet_in")?;
    let packet = Packet {
        value_ctx: state.value_ctx,
        packet_in,
        entrypoint,
    };
    let mut arch = get_arch_state(ctx, state.value_arch)?;
    match entrypoint {
        Entrypoint::Ingress => arch.queue.push_front(packet),
        Entrypoint::Egress => arch.queue.push_back(packet),
    }
    state.value_arch = set_arch_state(ctx, state.value_arch, &arch)?;
    Ok(())
}

pub fn schedule_resubmit<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    arch: &Arch,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let Some(index) = arch.action.resubmit_opt else {
        return Ok(false);
    };
    let value_ctx_original = state.value_ctx;
    prepare_resubmit_ctx(ctx, state, index)?;
    drive_pipe_pre(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    state.value_ctx = value_ctx_original;
    Ok(true)
}

pub fn schedule_clone<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    arch: &Arch,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let Some((clone_type, session, index)) = arch.action.clone_opt else {
        return Ok(false);
    };
    let Some(&port) = arch.mirrortable.get(&session) else {
        return Ok(false);
    };
    let value_ctx_original = state.value_ctx;
    prepare_clone_ctx(ctx, state, clone_type, port, index)?;
    if clone_type == CloneType::I2E {
        drive_pipe_pre(ctx, state)?;
    }
    schedule_packet(ctx, state, Entrypoint::Egress)?;
    state.value_ctx = value_ctx_original;
    Ok(true)
}

pub fn schedule_recirculate<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    arch: &Arch,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let Some(index) = arch.action.recirculate_opt else {
        return Ok(false);
    };
    let value_ctx_original = state.value_ctx;
    prepare_recirculate_ctx(ctx, state, index)?;
    // Run checksum and deparser before feeding the output back to the parser
    drive_ck(ctx, state)?;
    remove_packet_out(ctx, state)?;
    drive_dep(ctx, state)?;
    let packet = packet_string(ctx, state.value_arch)?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = put_object(ctx, state.value_arch, "packet_in", &pkt)?;
    // Parser and verify run before the ingress packet enters the queue
    drive_pipe_pre(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    state.value_ctx = value_ctx_original;
    Ok(true)
}

/// Preserve node handle order and each node's port order
pub fn schedule_multicast<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    arch: &Arch,
    group: i64,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
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

/// Ingress schedules clones before resubmit, multicast or drop handling
pub fn drive_ig<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let result = rel::v1model_ingress(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    let arch = get_arch_state(ctx, state.value_arch)?;
    schedule_clone(ctx, state, &arch)?;
    if schedule_resubmit(ctx, state, &arch)? {
        return Ok(());
    }
    let group = metadata_int(ctx, state, "mcast_grp")?;
    if group != 0 {
        schedule_multicast(ctx, state, &arch, group)?;
    } else if !is_dropped(ctx, state)? {
        schedule_packet(ctx, state, Entrypoint::Egress)?;
    }
    Ok(())
}

/// Assign egress_port to the destination port before egress processing
fn prepare_egress_ctx<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
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

/// Egress may stop this packet without discarding scheduler state or clones
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EgressOutcome {
    RunPost,
    SkipPost,
}

pub fn drive_eg<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<EgressOutcome, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    prepare_egress_ctx(ctx, state)?;
    let result = rel::v1model_egress(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    let arch = get_arch_state(ctx, state.value_arch)?;
    schedule_clone(ctx, state, &arch)?;
    if is_dropped(ctx, state)? {
        return Ok(EgressOutcome::SkipPost);
    }
    let arch = get_arch_state(ctx, state.value_arch)?;
    if schedule_recirculate(ctx, state, &arch)? {
        Ok(EgressOutcome::SkipPost)
    } else {
        Ok(EgressOutcome::RunPost)
    }
}

pub fn drive_packet<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    packet: Packet,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let entrypoint = packet.entrypoint;
    insert_packet(ctx, state, packet)?;
    match entrypoint {
        Entrypoint::Ingress => drive_ig(ctx, state),
        Entrypoint::Egress => match drive_eg(ctx, state)? {
            EgressOutcome::RunPost => drive_pipe_post(ctx, state),
            EgressOutcome::SkipPost => Ok(()),
        },
    }
}

/// Process one received packet and append transmissions in source order
pub fn drive_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    rx: &Transmission,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    setup_rx(ctx, state, rx)?;
    drive_pipe_pre(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    super::scheduler::run_scheduler(ctx, state)
}
