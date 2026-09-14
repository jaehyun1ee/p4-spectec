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
    object::{Counter, HashExtern, InternetChecksum, Meter, Register},
    packet::{Entrypoint, Packet},
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
        il::ast::Typ,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
};
use serde_derive_state::{DeserializeState, SerializeState};

pub struct Psa;

#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
/// Core and PSA-specific extern objects
pub enum ObjectState {
    PacketIn(PacketIn),
    PacketOut(PacketOut),
    Counter(Counter),
    Register(#[serde(state)] Register),
    Hash(HashExtern),
    InternetChecksum(InternetChecksum),
    Meter(Meter),
}

impl ObjectState {
    pub fn from_value(
        arena: &mut ValueArena,
        encoding: Encoding,
        value: &Value,
    ) -> Result<Self, ExternError> {
        let json = get::external_shared(arena, value)?.clone();
        decode_with(arena, encoding, json.as_ref())
            .map_err(|error| ExternError::Failure(error.to_string()))
    }

    pub fn to_value(
        &self,
        arena: &mut ValueArena,
        encoding: Encoding,
    ) -> Result<Value, ExternError> {
        let payload = encode_with(arena, encoding, self)
            .map_err(|error| ExternError::Failure(error.to_string()))?;
        external::state_value(arena, "objectState", payload.into())
    }
}

// Extern calls

impl Extern for Psa {
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
        let encoding = ctx.encoding();
        let value = match name {
            "init_archState" => Arch::default().to_value(ctx.arena_mut(), encoding)?,
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
                    "Counter" => Some(ObjectState::Counter(Counter::init(
                        ctx.arena(),
                        *value_targs,
                        *value_ids,
                        *value_args,
                    )?)),
                    "Register" => Some(ObjectState::Register(Register::init(
                        ctx,
                        *value_targs,
                        *value_ids,
                        *value_args,
                    )?)),
                    "Hash" => Some(ObjectState::Hash(HashExtern::init(
                        ctx.arena(),
                        *value_targs,
                        *value_ids,
                        *value_args,
                    )?)),
                    "InternetChecksum" => {
                        Some(ObjectState::InternetChecksum(InternetChecksum::init()))
                    }
                    "Meter" => Some(ObjectState::Meter(Meter::init(
                        ctx.arena(),
                        *value_targs,
                        *value_ids,
                        *value_args,
                    )?)),
                    _ => None,
                };
                match object {
                    Some(object) => object.to_value(ctx.arena_mut(), encoding)?,
                    None => {
                        let payload = encode_with(ctx.arena(), encoding, &())
                            .map_err(|error| ExternError::Failure(error.to_string()))?;
                        external::state_value(ctx.arena_mut(), "objectState", payload.into())?
                    }
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
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
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
    if name != "verify" || names != ["check", "toSignal"] {
        return Err(ExternError::Failure(format!(
            "unsupported extern function call: {name}({})",
            names.join(", ")
        ))
        .into());
    }
    let (value_ctx, value_arch, value_call_result) =
        core_func::verify(ctx, *value_ctx, *value_arch)?;
    Ok(vec![value_ctx, value_arch, value_call_result])
}

fn eval_method<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let encoding = ctx.encoding();
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
                let (object, value_ctx, value_arch, value_call_result) =
                    object.count(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Counter(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Register(object), "read", ["index"]) => {
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
            (ObjectState::Hash(object), "get_hash", ["data"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.get_hash(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Hash(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Hash(object), "get_hash", ["base", "data", "max"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.get_hash_adjust(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Hash(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::InternetChecksum(object), "clear", []) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.clear(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::InternetChecksum(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::InternetChecksum(object), "add", ["data"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.add(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::InternetChecksum(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::InternetChecksum(object), "subtract", ["data"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.subtract(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::InternetChecksum(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::InternetChecksum(object), "get", []) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.get(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::InternetChecksum(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::InternetChecksum(object), "get_state", []) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.get_state(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::InternetChecksum(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::InternetChecksum(object), "set_state", ["checksum_state"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.set_state(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::InternetChecksum(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Meter(object), "execute", ["index", "color"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.execute_color_aware(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Meter(object),
                    value_ctx,
                    value_arch,
                    value_call_result,
                )
            }
            (ObjectState::Meter(object), "execute", ["index"]) => {
                let (object, value_ctx, value_arch, value_call_result) =
                    object.execute_color_blind(ctx, *value_ctx, *value_arch)?;
                (
                    ObjectState::Meter(object),
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
    let encoding = ctx.encoding();
    let value_object = func::find_object_state_e(ctx, value_arch, value_id)?;
    Ok(ObjectState::from_value(
        ctx.arena_mut(),
        encoding,
        &value_object,
    )?)
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
    let encoding = ctx.encoding();
    let value_state = func::find_arch_state_e(ctx, value_arch)?;
    Ok(Arch::from_value(ctx.arena_mut(), encoding, &value_state)?)
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
    let encoding = ctx.encoding();
    let value_state = arch.to_value(ctx.arena_mut(), encoding)?;
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
    let encoding = ctx.encoding();
    let value_id = object_id(ctx.arena_mut(), name)?;
    let value_object = object.to_value(ctx.arena_mut(), encoding)?;
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
pub fn add_mirror_session_mc<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    session: i64,
    group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut arch = get_arch_state(ctx, value_arch)?;
    arch.mirrortable.insert(session, group);
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

// Register interface

fn get_register<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
) -> Result<Register, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value_id = object_id(ctx.arena_mut(), name)?;
    match get_object_state(ctx, value_arch, value_id)? {
        ObjectState::Register(reg) => Ok(reg),
        _ => Err(ExternError::Failure(format!("Register extern {name} not found")).into()),
    }
}

pub fn register_read<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
    idx: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let reg = get_register(ctx, value_arch, name)?;
    // Evaluate the register read; printing is disabled in the source
    if idx < reg.values.len() as i64 {
        let idx = usize::try_from(idx)
            .map_err(|_| ExternError::Failure("negative register index".to_owned()))?;
        reg.values
            .get(idx)
            .ok_or_else(|| ExternError::Failure("register index out of bounds".to_owned()))?;
    } else {
        func::default(ctx, reg.value_typ)?;
    }
    Ok(value_arch)
}

pub fn register_write<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
    idx: i64,
    int: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut reg = get_register(ctx, value_arch, name)?;
    let value = pack::p4_arbitrary_int(ctx.arena_mut(), int.into())?;
    let value = func::cast_op(ctx, reg.value_typ, value)?;
    for (idx_reg, value_reg) in reg.values.iter_mut().enumerate() {
        if idx_reg as i64 == idx {
            *value_reg = value;
        }
    }
    put_object(ctx, value_arch, name, &ObjectState::Register(reg))
}

pub fn register_reset<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut reg = get_register(ctx, value_arch, name)?;
    let value = func::default(ctx, reg.value_typ)?;
    reg.values.fill(value);
    put_object(ctx, value_arch, name, &ObjectState::Register(reg))
}

/// STF transformation
pub fn transform_stf_stmt(mut stmt: Statement) -> Statement {
    match &mut stmt {
        Statement::RegisterRead { name, .. }
        | Statement::RegisterWrite { name, .. }
        | Statement::RegisterReset { name } => {
            *name = name.clone().rewrite_substring(&["ingress"], "ip.ig");
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
    let (value_ctx, value_arch) = pgm::psa_init(ctx, program)?;
    Ok(SimState {
        value_ctx,
        value_arch,
        txs: vec![],
    })
}

fn metadata_bool<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &SimState,
    name: &str,
    field: &str,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value = rel::lvalue_read_dot_global(ctx, state.value_ctx, state.value_arch, name, field)?;
    Ok(unpack::p4_bool(ctx.arena(), &value)?)
}

fn metadata_int<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &SimState,
    name: &str,
    field: &str,
) -> Result<i64, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let value = rel::lvalue_read_dot_global(ctx, state.value_ctx, state.value_arch, name, field)?;
    let num = unpack::p4_fixed_bit(ctx.arena(), &value)?;
    Ok(unpack::signed_int(&num.int)?)
}

// Prepare egress context

fn prepare_egress<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    port: i64,
    path: &str,
    instance: i64,
    metadata: &str,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    // Prepare class of service
    let cos = metadata_int(ctx, state, metadata, "class_of_service")?;
    // Initialize egress metadata
    state.value_ctx = rel::psa_egress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        port,
        path,
        cos,
        instance,
    )?;
    Ok(())
}

// Packet state

fn reset_ingress_packet<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let mut pkt = get_packet_in(ctx, state.value_arch, "ingress_packet_in")?;
    pkt.reset();
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        "ingress_packet_in",
        &ObjectState::PacketIn(pkt),
    )?;
    Ok(())
}

fn reset_packet_out<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    name: &str,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        name,
        &ObjectState::PacketOut(PacketOut::default()),
    )?;
    Ok(())
}

fn packet_string<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    value_arch: Value,
    name_in: &str,
    name_out: &str,
) -> Result<String, Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let pkt_in = get_packet_in(ctx, value_arch, name_in)?;
    let pkt_out = get_packet_out(ctx, value_arch, name_out)?;
    Ok(core_packet::to_string(&pkt_in, &pkt_out)?)
}

/// Schedule a packet with its processing context
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
    let name = match entrypoint {
        Entrypoint::Ingress => "ingress_packet_in",
        Entrypoint::Egress => "egress_packet_in",
    };
    let packet_in = get_packet_in(ctx, state.value_arch, name)?;
    let packet = Packet {
        value_ctx: state.value_ctx,
        packet_in,
        entrypoint,
    };
    let mut arch = get_arch_state(ctx, state.value_arch)?;
    arch.queue.push_back(packet);
    state.value_arch = set_arch_state(ctx, state.value_arch, &arch)?;
    Ok(())
}

fn schedule_unicast<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let packet = packet_string(
        ctx,
        state.value_arch,
        "ingress_packet_in",
        "ingress_packet_out",
    )?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = put_object(ctx, state.value_arch, "egress_packet_in", &pkt)?;
    let port = metadata_int(ctx, state, "ingress_output_metadata", "egress_port")?;
    prepare_egress(
        ctx,
        state,
        port,
        "NORMAL_UNICAST",
        0,
        "ingress_output_metadata",
    )?;
    schedule_packet(ctx, state, Entrypoint::Egress)
}

/// Prepare and schedule multicast packets
pub fn schedule_multicast<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    group: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let arch = get_arch_state(ctx, state.value_arch)?;
    let Some(handles) = arch.multicast.groups.get(&group).cloned() else {
        return Ok(());
    };
    let packet = packet_string(
        ctx,
        state.value_arch,
        "ingress_packet_in",
        "ingress_packet_out",
    )?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = put_object(ctx, state.value_arch, "egress_packet_in", &pkt)?;
    let arch = get_arch_state(ctx, state.value_arch)?;
    for handle in handles {
        if let Some(nodes) = arch.multicast.nodes.get(&handle) {
            for node in nodes {
                let value_ctx_original = state.value_ctx;
                prepare_egress(
                    ctx,
                    state,
                    node.port,
                    "NORMAL_MULTICAST",
                    node.instance,
                    "ingress_output_metadata",
                )?;
                schedule_packet(ctx, state, Entrypoint::Egress)?;
                state.value_ctx = value_ctx_original;
            }
        }
    }
    Ok(())
}

fn schedule_clone<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    session: i64,
    origin: Entrypoint,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let arch = get_arch_state(ctx, state.value_arch)?;
    let Some(group) = arch.mirrortable.get(&session) else {
        return Ok(());
    };
    let Some(handles) = arch.multicast.groups.get(group).cloned() else {
        return Ok(());
    };
    // Preserve the original store for ingress or egress packet_in
    let value_arch_original = state.value_arch;
    let pkt = match origin {
        Entrypoint::Ingress => {
            reset_ingress_packet(ctx, state)?;
            get_packet_in(ctx, state.value_arch, "ingress_packet_in")?
        }
        Entrypoint::Egress => {
            let packet = packet_string(
                ctx,
                state.value_arch,
                "egress_packet_in",
                "egress_packet_out",
            )?;
            PacketIn::init(&packet)?
        }
    };
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        "egress_packet_in",
        &ObjectState::PacketIn(pkt),
    )?;
    let arch = get_arch_state(ctx, state.value_arch)?;
    let (path, metadata) = match origin {
        Entrypoint::Ingress => ("CLONE_I2E", "ingress_output_metadata"),
        Entrypoint::Egress => ("CLONE_E2E", "egress_input_metadata"),
    };
    for handle in handles {
        if let Some(nodes) = arch.multicast.nodes.get(&handle) {
            for node in nodes {
                let value_ctx_original = state.value_ctx;
                prepare_egress(ctx, state, node.port, path, node.instance, metadata)?;
                schedule_packet(ctx, state, Entrypoint::Egress)?;
                state.value_ctx = value_ctx_original;
            }
        }
    }
    let arch = get_arch_state(ctx, state.value_arch)?;
    // Restore the original store while retaining the current scheduler state
    state.value_arch = set_arch_state(ctx, value_arch_original, &arch)?;
    Ok(())
}

/// Prepare ingress metadata and resubmit the original ingress packet
pub fn schedule_resubmit<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let port = metadata_int(ctx, state, "ingress_input_metadata", "ingress_port")?;
    state.value_ctx =
        rel::psa_ingress_init_metadata(ctx, state.value_ctx, state.value_arch, port, "RESUBMIT")?;
    reset_ingress_packet(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)
}

/// Prepare and schedule an ingress packet from egress output
pub fn schedule_recirculate<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let packet = packet_string(
        ctx,
        state.value_arch,
        "egress_packet_in",
        "egress_packet_out",
    )?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = put_object(ctx, state.value_arch, "ingress_packet_in", &pkt)?;
    state.value_ctx = rel::psa_ingress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        0xfffffffa,
        "RECIRCULATE",
    )?;
    schedule_packet(ctx, state, Entrypoint::Ingress)
}

// Transfer the packet to its egress port

fn transfer_packet<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let port = metadata_int(ctx, state, "egress_input_metadata", "egress_port")?;
    let packet = packet_string(
        ctx,
        state.value_arch,
        "egress_packet_in",
        "egress_packet_out",
    )?;
    state.txs.push(Tx { port, packet });
    Ok(())
}

/// Packet replication engine
pub fn run_pre<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    if metadata_bool(ctx, state, "ingress_output_metadata", "clone")? {
        let session = metadata_int(ctx, state, "ingress_output_metadata", "clone_session_id")?;
        schedule_clone(ctx, state, session, Entrypoint::Ingress)?;
    }
    if metadata_bool(ctx, state, "ingress_output_metadata", "drop")? {
        return Ok(());
    }
    if metadata_bool(ctx, state, "ingress_output_metadata", "resubmit")? {
        return schedule_resubmit(ctx, state);
    }
    let group = metadata_int(ctx, state, "ingress_output_metadata", "multicast_group")?;
    if group != 0 {
        schedule_multicast(ctx, state, group)
    } else {
        schedule_unicast(ctx, state)
    }
}

/// Buffering queueing engine
pub fn run_bqe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    if metadata_bool(ctx, state, "egress_output_metadata", "clone")? {
        let session = metadata_int(ctx, state, "egress_output_metadata", "clone_session_id")?;
        schedule_clone(ctx, state, session, Entrypoint::Egress)?;
    }
    if metadata_bool(ctx, state, "egress_output_metadata", "drop")? {
        return Ok(());
    }
    let value_port = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "egress_input_metadata",
        "egress_port",
    )?;
    let num_port = unpack::p4_fixed_bit(ctx.arena(), &value_port)?;
    if num_port.width == 32.into() && num_port.int == 0xfffffffa_i64.into() {
        schedule_recirculate(ctx, state)
    } else {
        transfer_packet(ctx, state)
    }
}

/// Ingress pipeline driver
pub fn drive_ingress_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let (value_ctx, value_arch, value_call_result) =
        rel::psa_ingress_parser(ctx, state.value_ctx, state.value_arch)?;
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
            "ingress_input_metadata",
            "parser_error",
            value_error,
        )?;
    }
    let (value_ctx, value_arch, _) = rel::psa_ingress(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    reset_packet_out(ctx, state, "ingress_packet_out")?;
    let (value_ctx, value_arch, _) =
        rel::psa_ingress_deparser(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(())
}

/// Egress pipeline driver
pub fn drive_egress_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let (value_ctx, value_arch, value_call_result) =
        rel::psa_egress_parser(ctx, state.value_ctx, state.value_arch)?;
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
            "egress_input_metadata",
            "parser_error",
            value_error,
        )?;
    }
    let (value_ctx, value_arch, _) = rel::psa_egress(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    reset_packet_out(ctx, state, "egress_packet_out")?;
    let (value_ctx, value_arch, _) =
        rel::psa_egress_deparser(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(())
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
    let name = match packet.entrypoint {
        Entrypoint::Ingress => "ingress_packet_in",
        Entrypoint::Egress => "egress_packet_in",
    };
    state.value_arch = put_object(
        ctx,
        state.value_arch,
        name,
        &ObjectState::PacketIn(packet.packet_in),
    )?;
    state.value_ctx = packet.value_ctx;
    match packet.entrypoint {
        Entrypoint::Ingress => {
            drive_ingress_pipe(ctx, state)?;
            run_pre(ctx, state)
        }
        Entrypoint::Egress => {
            drive_egress_pipe(ctx, state)?;
            run_bqe(ctx, state)
        }
    }
}

/// Set up packets and globals, then schedule packets
pub fn drive_pipe<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
    rx: &Rx,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    let encoding = ctx.encoding();
    state.txs.clear();
    let pkt = ObjectState::PacketIn(PacketIn::init(&rx.packet)?);
    // Set up packet_in objects
    let value_packet = pkt.to_value(ctx.arena_mut(), encoding)?;
    let (value_ctx, value_arch) =
        rel::psa_ingress_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    let (value_ctx, value_arch) =
        rel::psa_egress_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    // Set up packet_out objects
    let value_packet =
        ObjectState::PacketOut(PacketOut::default()).to_value(ctx.arena_mut(), encoding)?;
    let (value_ctx, value_arch) =
        rel::psa_ingress_init_packet_out(ctx, state.value_ctx, state.value_arch, value_packet)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    let (value_ctx, value_arch) =
        rel::psa_egress_init_packet_out(ctx, state.value_ctx, state.value_arch, value_packet)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    // Set up global variables
    state.value_ctx =
        rel::psa_ingress_init_globals(ctx, state.value_ctx, state.value_arch, rx.port)?;
    state.value_ctx =
        rel::psa_egress_init_globals(ctx, state.value_ctx, state.value_arch, rx.port)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    super::scheduler::run_scheduler(ctx, state)
}
