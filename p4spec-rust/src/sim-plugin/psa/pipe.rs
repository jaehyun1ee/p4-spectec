//! PSA has a parser, control, and deparser for both ingress and egress
//!
//! ```text
//! Rx -> Ingress parser -> Ingress control -> Ingress deparser -> PRE
//!                                                               |
//!                                                               v
//!     Egress parser <- queued packet <--------------------------+
//!           |
//!           v
//!     Egress control -> Egress deparser -> BQE -> Tx
//! ```
//!
//! The packet replication engine (PRE) sends unicast, multicast, and ingress
//! clones to egress, or resubmits to ingress
//! The buffering queueing engine (BQE) sends egress clones to egress, or
//! recirculates to ingress
//!
//! Both engines can drop the current packet after scheduling its clones
//! The scheduler runs queued packets until none remain

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
    },
    runner::{ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
};
use serde_derive_state::{DeserializeState, SerializeState};

// == Configuration

#[derive(Default)]
pub struct Psa {
    encoding: Encoding,
}

impl Psa {
    pub fn new(encoding: Encoding) -> Self {
        Self { encoding }
    }
}

// == STF transformation

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

// == Architectural state

pub fn find_arch_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
) -> Result<Arch, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let encoding = ctx.external().encoding;
    let value_state = func::find_arch_state_e(ctx, value_arch)?;
    Ok(Arch::from_value(ctx.arena_mut(), encoding, &value_state)?)
}

pub fn update_arch_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    arch: &Arch,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let encoding = ctx.external().encoding;
    let value_state = arch.to_value(ctx.arena_mut(), encoding)?;
    func::update_arch_state_e(ctx, value_arch, value_state)
}

// == Extern objects

/// Core and PSA-specific extern objects
#[derive(Clone, Debug, PartialEq, Eq, SerializeState, DeserializeState)]
#[serde(serialize_state = "EncodeContext<'arena>", ser_parameters = "'arena")]
#[serde(deserialize_state = "DecodeContext<'de>")]
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

pub fn find_object_state<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    value_id: Value,
) -> Result<ObjectState, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let encoding = ctx.external().encoding;
    let value_object = func::find_object_state_e(ctx, value_arch, value_id)?;
    Ok(ObjectState::from_value(
        ctx.arena_mut(),
        encoding,
        &value_object,
    )?)
}

fn find_ingress_packet_in<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
) -> Result<PacketIn, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value_name = make::text(
        ctx.arena_mut(),
        "ingress_packet_in".to_owned(),
        Span::default(),
    )
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
        _ => Err(ExternError::Failure("ingress_packet_in extern not found".to_owned()).into()),
    }
}

fn find_ingress_packet_out<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
) -> Result<PacketOut, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value_name = make::text(
        ctx.arena_mut(),
        "ingress_packet_out".to_owned(),
        Span::default(),
    )
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
        _ => Err(ExternError::Failure("ingress_packet_out extern not found".to_owned()).into()),
    }
}

fn find_egress_packet_in<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
) -> Result<PacketIn, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value_name = make::text(
        ctx.arena_mut(),
        "egress_packet_in".to_owned(),
        Span::default(),
    )
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
        _ => Err(ExternError::Failure("egress_packet_in extern not found".to_owned()).into()),
    }
}

fn find_egress_packet_out<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
) -> Result<PacketOut, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value_name = make::text(
        ctx.arena_mut(),
        "egress_packet_out".to_owned(),
        Span::default(),
    )
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
        _ => Err(ExternError::Failure("egress_packet_out extern not found".to_owned()).into()),
    }
}

fn find_register<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    name: &str,
) -> Result<Register, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let values_name = name
        .split('.')
        .map(|name| make::text(ctx.arena_mut(), name.to_owned(), Span::default()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ExternError::from)?;
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
        ObjectState::Register(reg) => Ok(reg),
        _ => Err(ExternError::Failure(format!("Register extern {name} not found")).into()),
    }
}

fn update_register<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    name: &str,
    reg: Register,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let values_name = name
        .split('.')
        .map(|name| make::text(ctx.arena_mut(), name.to_owned(), Span::default()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(ExternError::from)?;
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
    let value_reg = ObjectState::Register(reg).to_value(ctx.arena_mut(), encoding)?;
    func::update_object_state_e(ctx, value_arch, value_id, value_reg)
}

// == Extern calls

impl external::Impl for Psa {
    fn eval_extern_init<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let encoding = self.encoding;
        let (value_name, value_targs, value_ids, value_args) =
            get::four(values).map_err(ExternError::from)?;
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
            "InternetChecksum" => Some(ObjectState::InternetChecksum(InternetChecksum::init())),
            "Meter" => Some(ObjectState::Meter(Meter::init(
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

    fn eval_extern_func_call<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Vec<Value>, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let (value_ctx, value_arch, value_name, value_names) =
            get::four(values).map_err(ExternError::from)?;
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

    fn eval_extern_method_call<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        values: &[Value],
    ) -> Result<Vec<Value>, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let encoding = self.encoding;
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

    fn init_arch_state<Interp, Iface>(
        &self,
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Arch::default()
            .to_value(ctx.arena_mut(), self.encoding)
            .map_err(Into::into)
    }
}

// == Mirror session interface

pub fn add_mirror_session_mc<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    session: i64,
    group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.mirrortable.insert(session, group);
    update_arch_state(ctx, value_arch, &arch)
}

// == Multicast interface

pub fn mc_mgrp_create<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    group: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.multicast.group_create(group);
    update_arch_state(ctx, value_arch, &arch)
}

pub fn mc_node_create<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    instance: i64,
    ports: &[i64],
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.multicast.node_create(instance, ports);
    update_arch_state(ctx, value_arch, &arch)
}

pub fn mc_node_associate<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    group: i64,
    handle: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut arch = find_arch_state(ctx, value_arch)?;
    arch.multicast.node_associate(group, handle);
    update_arch_state(ctx, value_arch, &arch)
}

// == Register interface

pub fn register_read<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    name: &str,
    idx: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let reg = find_register(ctx, value_arch, name)?;
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

pub fn register_write<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    name: &str,
    idx: i64,
    int: i64,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut reg = find_register(ctx, value_arch, name)?;
    let value = pack::p4_arbitrary_int(ctx.arena_mut(), int.into())?;
    let value = func::cast_op(ctx, reg.value_typ, value)?;
    for (idx_reg, value_reg) in reg.values.iter_mut().enumerate() {
        if idx_reg as i64 == idx {
            *value_reg = value;
        }
    }
    update_register(ctx, value_arch, name, reg)
}

pub fn register_reset<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    value_arch: Value,
    name: &str,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut reg = find_register(ctx, value_arch, name)?;
    let value = func::default(ctx, reg.value_typ)?;
    reg.values.fill(value);
    update_register(ctx, value_arch, name, reg)
}

// == Packet state

fn insert_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    packet: Packet,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let name = match packet.entrypoint {
        Entrypoint::Ingress => "ingress_packet_in",
        Entrypoint::Egress => "egress_packet_in",
    };
    state.value_arch = {
        let value_name = make::text(ctx.arena_mut(), name.to_owned(), Span::default())
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

fn remove_ingress_packet_in<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let mut pkt = find_ingress_packet_in(ctx, state.value_arch)?;
    pkt.reset();
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "ingress_packet_in".to_owned(),
            Span::default(),
        )
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

fn remove_ingress_packet_out<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "ingress_packet_out".to_owned(),
            Span::default(),
        )
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

fn remove_egress_packet_out<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "egress_packet_out".to_owned(),
            Span::default(),
        )
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

fn is_ingress_clone<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "ingress_output_metadata",
        "clone",
    )?;
    Ok(unpack::p4_bool(ctx.arena(), &value)?)
}

fn is_ingress_drop<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "ingress_output_metadata",
        "drop",
    )?;
    Ok(unpack::p4_bool(ctx.arena(), &value)?)
}

fn is_ingress_resubmit<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "ingress_output_metadata",
        "resubmit",
    )?;
    Ok(unpack::p4_bool(ctx.arena(), &value)?)
}

fn get_ingress_clone_session_id<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<i64, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "ingress_output_metadata",
        "clone_session_id",
    )?;
    let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
    Ok(unpack::signed_int(&int)?)
}

fn get_multicast_group<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<i64, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "ingress_output_metadata",
        "multicast_group",
    )?;
    let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
    Ok(unpack::signed_int(&int)?)
}

fn is_egress_clone<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "egress_output_metadata",
        "clone",
    )?;
    Ok(unpack::p4_bool(ctx.arena(), &value)?)
}

fn is_egress_drop<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "egress_output_metadata",
        "drop",
    )?;
    Ok(unpack::p4_bool(ctx.arena(), &value)?)
}

fn is_egress_recirculate<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<bool, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value_port = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "egress_input_metadata",
        "egress_port",
    )?;
    let (width_port, int_port) = unpack::p4_fixed_bit(ctx.arena(), &value_port)?;
    Ok(width_port == 32.into() && int_port == 0xfffffffa_i64.into())
}

fn get_egress_clone_session_id<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<i64, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let value = rel::lvalue_read_dot_global(
        ctx,
        state.value_ctx,
        state.value_arch,
        "egress_output_metadata",
        "clone_session_id",
    )?;
    let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
    Ok(unpack::signed_int(&int)?)
}

// == Pipeline initializer

pub fn init_pipe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    program: Value,
) -> Result<SimState, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let (value_ctx, value_arch) = pgm::psa_init(ctx, program)?;
    Ok(SimState {
        value_ctx,
        value_arch,
        txs: vec![],
    })
}

// == Prepare context

fn prepare_unicast_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let port = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "ingress_output_metadata",
            "egress_port",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    let cos = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "ingress_output_metadata",
            "class_of_service",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    state.value_ctx = rel::psa_egress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        port,
        "NORMAL_UNICAST",
        cos,
        0,
    )?;
    Ok(())
}

fn prepare_multicast_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    instance: i64,
    port: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let cos = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "ingress_output_metadata",
            "class_of_service",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    state.value_ctx = rel::psa_egress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        port,
        "NORMAL_MULTICAST",
        cos,
        instance,
    )?;
    Ok(())
}

fn prepare_clone_i2e_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    instance: i64,
    port: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let cos = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "ingress_output_metadata",
            "class_of_service",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    state.value_ctx = rel::psa_egress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        port,
        "CLONE_I2E",
        cos,
        instance,
    )?;
    Ok(())
}

fn prepare_clone_e2e_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    instance: i64,
    port: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let cos = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "egress_input_metadata",
            "class_of_service",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    state.value_ctx = rel::psa_egress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        port,
        "CLONE_E2E",
        cos,
        instance,
    )?;
    Ok(())
}

fn prepare_resubmit_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let port = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "ingress_input_metadata",
            "ingress_port",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    state.value_ctx =
        rel::psa_ingress_init_metadata(ctx, state.value_ctx, state.value_arch, port, "RESUBMIT")?;
    Ok(())
}

fn prepare_recirculate_ctx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    state.value_ctx = rel::psa_ingress_init_metadata(
        ctx,
        state.value_ctx,
        state.value_arch,
        0xfffffffa,
        "RECIRCULATE",
    )?;
    Ok(())
}

// == Schedule packet

pub fn schedule_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    entrypoint: Entrypoint,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let packet_in = match entrypoint {
        Entrypoint::Ingress => find_ingress_packet_in(ctx, state.value_arch)?,
        Entrypoint::Egress => find_egress_packet_in(ctx, state.value_arch)?,
    };
    let packet = Packet {
        value_ctx: state.value_ctx,
        packet_in,
        entrypoint,
    };
    let mut arch = find_arch_state(ctx, state.value_arch)?;
    arch.queue.push_back(packet);
    state.value_arch = update_arch_state(ctx, state.value_arch, &arch)?;
    Ok(())
}

fn schedule_unicast<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let packet = {
        let pkt_in = find_ingress_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_ingress_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "egress_packet_in".to_owned(),
            Span::default(),
        )
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
    prepare_unicast_ctx(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Egress)
}

pub fn schedule_multicast<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    group: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let arch = find_arch_state(ctx, state.value_arch)?;
    let Some(handles) = arch.multicast.groups.get(&group).cloned() else {
        return Ok(());
    };
    let packet = {
        let pkt_in = find_ingress_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_ingress_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "egress_packet_in".to_owned(),
            Span::default(),
        )
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
    let arch = find_arch_state(ctx, state.value_arch)?;
    for handle in handles {
        if let Some(nodes) = arch.multicast.nodes.get(&handle) {
            for node in nodes {
                let value_ctx_original = state.value_ctx;
                prepare_multicast_ctx(ctx, state, node.instance, node.port)?;
                schedule_packet(ctx, state, Entrypoint::Egress)?;
                state.value_ctx = value_ctx_original;
            }
        }
    }
    Ok(())
}

fn schedule_clone_i2e<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    session: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let arch = find_arch_state(ctx, state.value_arch)?;
    let Some(group) = arch.mirrortable.get(&session) else {
        return Ok(());
    };
    let Some(handles) = arch.multicast.groups.get(group).cloned() else {
        return Ok(());
    };
    // Preserve the original store for ingress packet_in
    let value_arch_original = state.value_arch;
    remove_ingress_packet_in(ctx, state)?;
    let pkt = find_ingress_packet_in(ctx, state.value_arch)?;
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "egress_packet_in".to_owned(),
            Span::default(),
        )
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
    let arch = find_arch_state(ctx, state.value_arch)?;
    for handle in handles {
        if let Some(nodes) = arch.multicast.nodes.get(&handle) {
            for node in nodes {
                let value_ctx_original = state.value_ctx;
                prepare_clone_i2e_ctx(ctx, state, node.instance, node.port)?;
                schedule_packet(ctx, state, Entrypoint::Egress)?;
                state.value_ctx = value_ctx_original;
            }
        }
    }
    let arch = find_arch_state(ctx, state.value_arch)?;
    // Restore the original store while retaining the current scheduler state
    state.value_arch = update_arch_state(ctx, value_arch_original, &arch)?;
    Ok(())
}

fn schedule_clone_e2e<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    session: i64,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let arch = find_arch_state(ctx, state.value_arch)?;
    let Some(group) = arch.mirrortable.get(&session) else {
        return Ok(());
    };
    let Some(handles) = arch.multicast.groups.get(group).cloned() else {
        return Ok(());
    };
    // Preserve the original store for egress packet_in
    let value_arch_original = state.value_arch;
    let packet = {
        let pkt_in = find_egress_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_egress_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    let pkt = PacketIn::init(&packet)?;
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "egress_packet_in".to_owned(),
            Span::default(),
        )
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
    let arch = find_arch_state(ctx, state.value_arch)?;
    for handle in handles {
        if let Some(nodes) = arch.multicast.nodes.get(&handle) {
            for node in nodes {
                let value_ctx_original = state.value_ctx;
                prepare_clone_e2e_ctx(ctx, state, node.instance, node.port)?;
                schedule_packet(ctx, state, Entrypoint::Egress)?;
                state.value_ctx = value_ctx_original;
            }
        }
    }
    let arch = find_arch_state(ctx, state.value_arch)?;
    // Restore the original store while retaining the current scheduler state
    state.value_arch = update_arch_state(ctx, value_arch_original, &arch)?;
    Ok(())
}

pub fn schedule_resubmit<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    prepare_resubmit_ctx(ctx, state)?;
    remove_ingress_packet_in(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)
}

pub fn schedule_recirculate<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let packet = {
        let pkt_in = find_egress_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_egress_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    let pkt = ObjectState::PacketIn(PacketIn::init(&packet)?);
    state.value_arch = {
        let value_name = make::text(
            ctx.arena_mut(),
            "ingress_packet_in".to_owned(),
            Span::default(),
        )
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
    prepare_recirculate_ctx(ctx, state)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)
}

fn transfer_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let port = {
        let value = rel::lvalue_read_dot_global(
            ctx,
            state.value_ctx,
            state.value_arch,
            "egress_input_metadata",
            "egress_port",
        )?;
        let (_, int) = unpack::p4_fixed_bit(ctx.arena(), &value)?;
        unpack::signed_int(&int)
    }?;
    let packet = {
        let pkt_in = find_egress_packet_in(ctx, state.value_arch)?;
        let pkt_out = find_egress_packet_out(ctx, state.value_arch)?;
        core_packet::to_string(&pkt_in, &pkt_out)
    }?;
    state.txs.push(Tx { port, packet });
    Ok(())
}

// == Setup packets and globals

fn setup_rx<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    rx: &Rx,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let encoding = ctx.external().encoding;
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
    Ok(())
}

// == Ingress pipeline driver

fn drive_ip<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
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
    Ok(())
}

fn drive_ig<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let (value_ctx, value_arch, value_result) =
        rel::psa_ingress(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

fn drive_id<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let (value_ctx, value_arch, value_result) =
        rel::psa_ingress_deparser(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

pub fn drive_ingress_pipe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    drive_ip(ctx, state)?;
    drive_ig(ctx, state)?;
    remove_ingress_packet_out(ctx, state)?;
    drive_id(ctx, state)
}

// == Packet replication engine

pub fn run_pre<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    if is_ingress_clone(ctx, state)? {
        let session = get_ingress_clone_session_id(ctx, state)?;
        schedule_clone_i2e(ctx, state, session)?;
    }
    if is_ingress_drop(ctx, state)? {
        return Ok(());
    }
    if is_ingress_resubmit(ctx, state)? {
        return schedule_resubmit(ctx, state);
    }
    let group = get_multicast_group(ctx, state)?;
    if group != 0 {
        schedule_multicast(ctx, state, group)
    } else {
        schedule_unicast(ctx, state)
    }
}

// == Egress pipeline driver

fn drive_ep<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
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
    Ok(())
}

fn drive_eg<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let (value_ctx, value_arch, value_result) =
        rel::psa_egress(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

fn drive_ed<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let (value_ctx, value_arch, value_result) =
        rel::psa_egress_deparser(ctx, state.value_ctx, state.value_arch)?;
    (state.value_ctx, state.value_arch) = (value_ctx, value_arch);
    Ok(value_result)
}

pub fn drive_egress_pipe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<Value, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    drive_ep(ctx, state)?;
    drive_eg(ctx, state)?;
    remove_egress_packet_out(ctx, state)?;
    drive_ed(ctx, state)
}

// == Buffering queueing engine

pub fn run_bqe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    if is_egress_clone(ctx, state)? {
        let session = get_egress_clone_session_id(ctx, state)?;
        schedule_clone_e2e(ctx, state, session)?;
    }
    if is_egress_drop(ctx, state)? {
        return Ok(());
    }
    if is_egress_recirculate(ctx, state)? {
        schedule_recirculate(ctx, state)
    } else {
        transfer_packet(ctx, state)
    }
}

// == Scheduling packets

pub fn drive_packet<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    packet: Packet,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    let entrypoint = packet.entrypoint;
    insert_packet(ctx, state, packet)?;
    match entrypoint {
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

pub fn run_scheduler<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    loop {
        let mut arch = find_arch_state(ctx, state.value_arch)?;
        let Some(packet) = arch.queue.pop_front() else {
            return Ok(());
        };
        state.value_arch = update_arch_state(ctx, state.value_arch, &arch)?;
        drive_packet(ctx, state, packet)?;
    }
}

pub fn drive_pipe<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
    rx: &Rx,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    state.txs.clear();
    setup_rx(ctx, state, rx)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    run_scheduler(ctx, state)
}
