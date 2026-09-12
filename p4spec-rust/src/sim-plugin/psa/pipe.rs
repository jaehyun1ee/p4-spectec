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
    object::{Counter, HashExtern, InternetChecksum, Meter, Register},
    packet::{Entrypoint, Packet},
};
use crate::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make},
        },
        il::ast::Typ,
    },
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
    util::json::json,
};

pub struct Psa;

#[derive(Clone, Debug, PartialEq, Eq)]
/// Core and PSA-specific extern objects
pub enum ObjectState {
    PacketIn(PacketIn),
    PacketOut(PacketOut),
    Counter(Counter),
    Register(Register),
    Hash(HashExtern),
    InternetChecksum(InternetChecksum),
    Meter(Meter),
}

impl ObjectState {
    pub fn from_value(arena: &mut ValueArena, value: &Value) -> Result<Self, ExternError> {
        let json = get::external(arena, value)?.clone();
        let fields = json
            .as_object()
            .filter(|fields| fields.len() == 1)
            .ok_or_else(|| ExternError::Failure("expected PSA object variant".to_owned()))?;
        let (name, json) = fields.iter().next().expect("one object variant");
        match name.as_str() {
            "PacketIn" => Ok(Self::PacketIn(PacketIn::from_json(json)?)),
            "PacketOut" => serde_json::from_value(json.clone()).map(Self::PacketOut),
            "Counter" => serde_json::from_value(json.clone()).map(Self::Counter),
            "Register" => return Register::from_json(arena, json).map(Self::Register),
            "Hash" => serde_json::from_value(json.clone()).map(Self::Hash),
            "InternetChecksum" => serde_json::from_value(json.clone()).map(Self::InternetChecksum),
            "Meter" => serde_json::from_value(json.clone()).map(Self::Meter),
            _ => return Err(ExternError::Failure(format!("unknown PSA object: {name}"))),
        }
        .map_err(|error| ExternError::Failure(error.to_string()))
    }

    pub fn to_value(&self, arena: &mut ValueArena) -> Result<Value, ExternError> {
        let (name, json) = match self {
            Self::PacketIn(pkt) => ("PacketIn", serde_json::to_value(pkt)),
            Self::PacketOut(pkt) => ("PacketOut", serde_json::to_value(pkt)),
            Self::Counter(counter) => ("Counter", serde_json::to_value(counter)),
            Self::Register(reg) => ("Register", Ok(reg.to_json(arena)?)),
            Self::Hash(hash) => ("Hash", serde_json::to_value(hash)),
            Self::InternetChecksum(checksum) => {
                ("InternetChecksum", serde_json::to_value(checksum))
            }
            Self::Meter(meter) => ("Meter", serde_json::to_value(meter)),
        };
        let json = json.map_err(|error| ExternError::Failure(error.to_string()))?;
        external::state_value(arena, "objectState", serde_json::json!({name: json}))
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
    let result = core_func::verify(ctx, *value_ctx, *value_arch)?;
    Ok(vec![
        result.value_ctx,
        result.value_arch,
        result.value_call_result,
    ])
}

fn eval_method<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    values: &[Value],
) -> Result<Vec<Value>, Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
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
            let output = object.count(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Counter(output.object), output.result)
        }
        (ObjectState::Register(object), "read", ["index"]) => {
            let output = object.read(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Register(output.object), output.result)
        }
        (ObjectState::Register(object), "write", ["index", "value"]) => {
            let output = object.write(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Register(output.object), output.result)
        }
        (ObjectState::Hash(object), "get_hash", ["data"]) => {
            let output = object.get_hash(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Hash(output.object), output.result)
        }
        (ObjectState::Hash(object), "get_hash", ["base", "data", "max"]) => {
            let output = object.get_hash_adjust(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Hash(output.object), output.result)
        }
        (ObjectState::InternetChecksum(object), "clear", []) => {
            let output = object.clear(ctx, *value_ctx, *value_arch)?;
            (ObjectState::InternetChecksum(output.object), output.result)
        }
        (ObjectState::InternetChecksum(object), "add", ["data"]) => {
            let output = object.add(ctx, *value_ctx, *value_arch)?;
            (ObjectState::InternetChecksum(output.object), output.result)
        }
        (ObjectState::InternetChecksum(object), "subtract", ["data"]) => {
            let output = object.subtract(ctx, *value_ctx, *value_arch)?;
            (ObjectState::InternetChecksum(output.object), output.result)
        }
        (ObjectState::InternetChecksum(object), "get", []) => {
            let output = object.get(ctx, *value_ctx, *value_arch)?;
            (ObjectState::InternetChecksum(output.object), output.result)
        }
        (ObjectState::InternetChecksum(object), "get_state", []) => {
            let output = object.get_state(ctx, *value_ctx, *value_arch)?;
            (ObjectState::InternetChecksum(output.object), output.result)
        }
        (ObjectState::InternetChecksum(object), "set_state", ["checksum_state"]) => {
            let output = object.set_state(ctx, *value_ctx, *value_arch)?;
            (ObjectState::InternetChecksum(output.object), output.result)
        }
        (ObjectState::Meter(object), "execute", ["index", "color"]) => {
            let output = object.execute_color_aware(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Meter(output.object), output.result)
        }
        (ObjectState::Meter(object), "execute", ["index"]) => {
            let output = object.execute_color_blind(ctx, *value_ctx, *value_arch)?;
            (ObjectState::Meter(output.object), output.result)
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
    let result = pgm::psa_init(ctx, program)?;
    Ok(SimState {
        value_ctx: result.value_ctx,
        value_arch: result.value_arch,
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
    Ok(core_object::packet_to_string(&pkt_in, &pkt_out)?)
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
    state.txs.push(Transmission { port, packet });
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
    let result = rel::psa_ingress_parser(ctx, state.value_ctx, state.value_arch)?;
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
            "ingress_input_metadata",
            "parser_error",
            value_error,
        )?;
    }
    let result = rel::psa_ingress(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    reset_packet_out(ctx, state, "ingress_packet_out")?;
    let result = rel::psa_ingress_deparser(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
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
    let result = rel::psa_egress_parser(ctx, state.value_ctx, state.value_arch)?;
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
            "egress_input_metadata",
            "parser_error",
            value_error,
        )?;
    }
    let result = rel::psa_egress(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
    reset_packet_out(ctx, state, "egress_packet_out")?;
    let result = rel::psa_egress_deparser(ctx, state.value_ctx, state.value_arch)?;
    install_result!(state, result);
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
    rx: &Transmission,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
{
    state.txs.clear();
    let pkt = ObjectState::PacketIn(PacketIn::init(&rx.packet)?);
    // Set up packet_in objects
    let value_packet = pkt.to_value(ctx.arena_mut())?;
    let result =
        rel::psa_ingress_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    let result =
        rel::psa_egress_init_packet_in(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    // Set up packet_out objects
    let value_packet = ObjectState::PacketOut(PacketOut::default()).to_value(ctx.arena_mut())?;
    let result =
        rel::psa_ingress_init_packet_out(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    let result =
        rel::psa_egress_init_packet_out(ctx, state.value_ctx, state.value_arch, value_packet)?;
    install_result!(state, result);
    // Set up global variables
    state.value_ctx =
        rel::psa_ingress_init_globals(ctx, state.value_ctx, state.value_arch, rx.port)?;
    state.value_ctx =
        rel::psa_egress_init_globals(ctx, state.value_ctx, state.value_arch, rx.port)?;
    schedule_packet(ctx, state, Entrypoint::Ingress)?;
    super::scheduler::run_scheduler(ctx, state)
}
