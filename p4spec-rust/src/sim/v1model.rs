use std::collections::{BTreeMap, VecDeque};

use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::{
    domain::external_data::ExternalData,
    interface::{Extern, ExternError, SpecCall},
    lang::il::ast::Typ,
    runtime::value::{ValueKind, ValueRef, get},
};

use super::{
    SimError,
    architecture::Architecture,
    core::{
        PacketIn, PacketOut, extern_error, external_value, pack_p4_arbitrary_int,
        pack_p4_fixed_bit, packet_advance, packet_extract, packet_lookahead, reject_result,
        return_result, sim_extern_error, text_list, unpack_p4_bool, unpack_p4_enum,
        unpack_p4_fixed_bit, unpack_p4_string, unpack_p4_tuple, unsupported_method,
        value_extern_error,
    },
    hash::compute_checksum,
    io::{Rx, Tx},
    psa::{Counter, Register, decode_bigint, decode_value, encode_bigint, encode_value},
    spec::{Spec, global_cursor, local_cursor, prefixed_name, storage_reference},
};
use crate::wire::sim_suite::{StfAction, StfMatch, StfStmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CloneKind {
    I2E,
    E2E,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CloneAction {
    kind: CloneKind,
    session: i32,
    index: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct V1Multicast {
    next_handle: i32,
    groups: BTreeMap<i32, Vec<i32>>,
    nodes: BTreeMap<i32, Vec<(i32, i32)>>,
}

impl V1Multicast {
    fn create_group(&mut self, group: i32) {
        self.groups.insert(group, Vec::new());
    }

    fn create_node(&mut self, rid: i32, ports: Vec<i32>) {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.nodes
            .insert(handle, ports.into_iter().map(|port| (port, rid)).collect());
    }

    fn associate(&mut self, group: i32, handle: i32) {
        if let Some(handles) = self.groups.get_mut(&group) {
            handles.insert(0, handle);
        }
    }

    fn replicas(&self, group: i32) -> Vec<(i32, i32)> {
        self.groups
            .get(&group)
            .into_iter()
            .flatten()
            .filter_map(|handle| self.nodes.get(handle))
            .flatten()
            .copied()
            .collect()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ArchitectureState {
    mirrors: BTreeMap<i32, i32>,
    multicast: V1Multicast,
    clone_action: Option<CloneAction>,
    resubmit: Option<i32>,
    recirculate: Option<i32>,
}

impl ArchitectureState {
    fn reset_actions(&mut self) {
        self.clone_action = None;
        self.resubmit = None;
        self.recirculate = None;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DirectCounter {
    Packets(BigInt),
    Bytes(BigInt),
    PacketsAndBytes(BigInt, BigInt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DirectMeter {
    Packets(BigInt),
    Bytes(BigInt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum V1Object {
    PacketIn(PacketIn),
    PacketOut(PacketOut),
    Counter(Counter),
    Register(Register),
    DirectCounter(DirectCounter),
    DirectMeter(DirectMeter),
}

impl V1Object {
    fn to_external(&self) -> Result<ExternalData, SimError> {
        let (kind, fields) = match self {
            Self::PacketIn(packet) => return Ok(packet.to_external()),
            Self::PacketOut(packet) => return Ok(packet.to_external()),
            Self::Counter(counter) => {
                let (counter_kind, values) = match counter {
                    Counter::Packets(values) => ("packets", encode_bigint_list(values)),
                    Counter::Bytes(values) => ("bytes", encode_bigint_list(values)),
                    Counter::PacketsAndBytes(values) => (
                        "packets-and-bytes",
                        ExternalData::List(
                            values
                                .iter()
                                .map(|(packets, bytes)| {
                                    ExternalData::Tuple(vec![
                                        encode_bigint(packets),
                                        encode_bigint(bytes),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                };
                (
                    "v1-counter",
                    vec![
                        (
                            "counter-kind".to_owned(),
                            ExternalData::String(counter_kind.to_owned()),
                        ),
                        ("values".to_owned(), values),
                    ],
                )
            }
            Self::Register(register) => (
                "v1-register",
                vec![
                    ("type".to_owned(), encode_value(register.typ())?),
                    (
                        "values".to_owned(),
                        ExternalData::List(
                            register
                                .values()
                                .iter()
                                .map(encode_value)
                                .collect::<Result<Vec<_>, _>>()?,
                        ),
                    ),
                ],
            ),
            Self::DirectCounter(counter) => {
                let (counter_kind, values) = match counter {
                    DirectCounter::Packets(value) => ("packets", vec![encode_bigint(value)]),
                    DirectCounter::Bytes(value) => ("bytes", vec![encode_bigint(value)]),
                    DirectCounter::PacketsAndBytes(packets, bytes) => (
                        "packets-and-bytes",
                        vec![encode_bigint(packets), encode_bigint(bytes)],
                    ),
                };
                (
                    "v1-direct-counter",
                    vec![
                        (
                            "counter-kind".to_owned(),
                            ExternalData::String(counter_kind.to_owned()),
                        ),
                        ("values".to_owned(), ExternalData::List(values)),
                    ],
                )
            }
            Self::DirectMeter(meter) => {
                let (meter_kind, value) = match meter {
                    DirectMeter::Packets(value) => ("packets", value),
                    DirectMeter::Bytes(value) => ("bytes", value),
                };
                (
                    "v1-direct-meter",
                    vec![
                        (
                            "meter-kind".to_owned(),
                            ExternalData::String(meter_kind.to_owned()),
                        ),
                        ("value".to_owned(), encode_bigint(value)),
                    ],
                )
            }
        };
        let mut all_fields = vec![("kind".to_owned(), ExternalData::String(kind.to_owned()))];
        all_fields.extend(fields);
        Ok(ExternalData::Assoc(all_fields))
    }

    fn from_external(value: &ExternalData) -> Result<Self, SimError> {
        let ExternalData::Assoc(fields) = value else {
            return Err(SimError::message("expected v1model object state"));
        };
        let ExternalData::String(kind) = field(fields, "kind")? else {
            return Err(SimError::message("v1model object kind must be a string"));
        };
        match kind.as_str() {
            "packet-in" => PacketIn::from_external(value).map(Self::PacketIn),
            "packet-out" => PacketOut::from_external(value).map(Self::PacketOut),
            "v1-counter" => decode_counter(fields).map(Self::Counter),
            "v1-register" => {
                let typ = decode_value(field(fields, "type")?)?;
                let ExternalData::List(values) = field(fields, "values")? else {
                    return Err(SimError::message("register values must be a list"));
                };
                let values = values
                    .iter()
                    .map(decode_value)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Register(Register::from_values(typ, values)))
            }
            "v1-direct-counter" => {
                let counter_kind = string_field(fields, "counter-kind")?;
                let ExternalData::List(values) = field(fields, "values")? else {
                    return Err(SimError::message("direct counter values must be a list"));
                };
                let counter = match (counter_kind, values.as_slice()) {
                    ("packets", [value]) => DirectCounter::Packets(decode_bigint(value)?),
                    ("bytes", [value]) => DirectCounter::Bytes(decode_bigint(value)?),
                    ("packets-and-bytes", [packets, bytes]) => DirectCounter::PacketsAndBytes(
                        decode_bigint(packets)?,
                        decode_bigint(bytes)?,
                    ),
                    _ => return Err(SimError::message("invalid direct counter state")),
                };
                Ok(Self::DirectCounter(counter))
            }
            "v1-direct-meter" => {
                let value = decode_bigint(field(fields, "value")?)?;
                match string_field(fields, "meter-kind")? {
                    "packets" => Ok(Self::DirectMeter(DirectMeter::Packets(value))),
                    "bytes" => Ok(Self::DirectMeter(DirectMeter::Bytes(value))),
                    _ => Err(SimError::message("invalid direct meter state")),
                }
            }
            _ => Err(SimError::message(format!(
                "unknown v1model object state `{kind}`"
            ))),
        }
    }
}

pub struct V1Model;

impl V1Model {
    pub fn new() -> Self {
        Self
    }

    fn eval_extern_init(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        let [name, type_args, ids, arguments] = values else {
            return Err(extern_error(
                "unexpected number of arguments to extern init",
            ));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let ids = text_list(ids)?;
        let arguments = get::list(arguments).map_err(value_extern_error)?;
        if ids.len() != arguments.len() {
            return Err(extern_error("extern argument name/value count mismatch"));
        }
        let argument = |name: &str| {
            ids.iter()
                .position(|id| id == name)
                .and_then(|index| arguments.get(index))
                .ok_or_else(|| extern_error(format!("missing {name} argument")))
        };
        let mut bridge = Spec::new(spec);
        let object = match name {
            "counter" => {
                let size = fixed_usize(argument("size")?)?;
                let (enum_type, variant) =
                    unpack_p4_enum(argument("type")?).map_err(sim_extern_error)?;
                if enum_type != "CounterType" {
                    return Err(extern_error("invalid CounterType"));
                }
                let counter = match variant.as_str() {
                    "packets" => Counter::packets(size),
                    "bytes" => Counter::bytes(size),
                    "packets_and_bytes" => Counter::packets_and_bytes(size),
                    _ => return Err(extern_error("invalid CounterType")),
                };
                V1Object::Counter(counter)
            }
            "register" => {
                let type_args = get::list(type_args).map_err(value_extern_error)?;
                let [typ] = type_args else {
                    return Err(extern_error("register constructor expects 1 type argument"));
                };
                let size = fixed_usize(argument("size")?)?;
                let initial = bridge.default_value(typ).map_err(sim_extern_error)?;
                V1Object::Register(Register::new(typ.clone(), size, initial))
            }
            "direct_counter" => {
                let (enum_type, variant) =
                    unpack_p4_enum(argument("type")?).map_err(sim_extern_error)?;
                if enum_type != "CounterType" {
                    return Err(extern_error("invalid CounterType"));
                }
                let counter = match variant.as_str() {
                    "packets" => DirectCounter::Packets(BigInt::zero()),
                    "bytes" => DirectCounter::Bytes(BigInt::zero()),
                    "packets_and_bytes" => {
                        DirectCounter::PacketsAndBytes(BigInt::zero(), BigInt::zero())
                    }
                    _ => return Err(extern_error("invalid CounterType")),
                };
                V1Object::DirectCounter(counter)
            }
            "direct_meter" => {
                let (enum_type, variant) =
                    unpack_p4_enum(argument("type")?).map_err(sim_extern_error)?;
                if enum_type != "MeterType" {
                    return Err(extern_error("invalid MeterType"));
                }
                let meter = match variant.as_str() {
                    "packets" => DirectMeter::Packets(BigInt::zero()),
                    "bytes" => DirectMeter::Bytes(BigInt::zero()),
                    _ => return Err(extern_error("invalid MeterType")),
                };
                V1Object::DirectMeter(meter)
            }
            _ => return Ok(external_value("objectState", ExternalData::Null)),
        };
        Ok(external_value(
            "objectState",
            object.to_external().map_err(sim_extern_error)?,
        ))
    }

    fn eval_compile_time_function(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        let [context, name, parameters] = values else {
            return Err(extern_error("unexpected compile-time extern call shape"));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let parameters = text_list(parameters)?;
        let has_message = match (name, parameters.as_slice()) {
            ("static_assert", [check, message]) if check == "check" && message == "message" => true,
            ("static_assert", [check]) if check == "check" => false,
            _ => return Err(extern_error("unsupported compile-time extern function")),
        };
        let mut bridge = Spec::new(spec);
        let cursor = local_cursor().map_err(sim_extern_error)?;
        let check = bridge
            .find_var_value(&cursor, context, "check")
            .map_err(sim_extern_error)?;
        if unpack_p4_bool(&check).map_err(sim_extern_error)? {
            return Ok(vec![check]);
        }
        let message = if has_message {
            let message = bridge
                .find_var_value(&cursor, context, "message")
                .map_err(sim_extern_error)?;
            super::core::unpack_p4_string(&message).map_err(sim_extern_error)?
        } else {
            "static_assert failed".to_owned()
        };
        Err(extern_error(message))
    }

    fn eval_extern_function(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        let [context, architecture, name, parameters] = values else {
            return Err(extern_error("unexpected extern function call shape"));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let parameters = text_list(parameters)?;
        let mut bridge = Spec::new(spec);
        let cursor = local_cursor().map_err(sim_extern_error)?;
        let (context, architecture, result) = match (name, parameters.as_slice()) {
            ("verify", [check, signal]) if check == "check" && signal == "toSignal" => {
                let check = bridge
                    .find_var(&cursor, context, "check")
                    .map_err(sim_extern_error)?;
                let signal = bridge
                    .find_var(&cursor, context, "toSignal")
                    .map_err(sim_extern_error)?;
                let result = if unpack_p4_bool(&check).map_err(sim_extern_error)? {
                    return_result(None).map_err(sim_extern_error)?
                } else {
                    reject_result(signal, "rejectResult").map_err(sim_extern_error)?
                };
                (context.clone(), architecture.clone(), result)
            }
            ("digest", [receiver, data]) if receiver == "receiver" && data == "data" => (
                context.clone(),
                architecture.clone(),
                return_result(None).map_err(sim_extern_error)?,
            ),
            ("mark_to_drop", [metadata]) if metadata == "standard_metadata" => {
                let context = write_local_member(
                    &mut bridge,
                    context,
                    architecture,
                    "standard_metadata",
                    "egress_spec",
                    &pack_p4_fixed_bit(BigInt::from(9), BigInt::from(511))
                        .map_err(sim_extern_error)?,
                )?;
                let context = write_local_member(
                    &mut bridge,
                    &context,
                    architecture,
                    "standard_metadata",
                    "mcast_grp",
                    &pack_p4_fixed_bit(BigInt::from(16), BigInt::zero())
                        .map_err(sim_extern_error)?,
                )?;
                (
                    context,
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("hash", [result, algo, base, data, maximum])
                if result == "result"
                    && algo == "algo"
                    && base == "base"
                    && data == "data"
                    && maximum == "max" =>
            {
                let context = eval_hash(&mut bridge, context, architecture)?;
                (
                    context,
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("verify_checksum", [condition, data, checksum, algo])
                if condition == "condition"
                    && data == "data"
                    && checksum == "checksum"
                    && algo == "algo" =>
            {
                let context = eval_verify_checksum(&mut bridge, context, architecture, None)?;
                (
                    context,
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("verify_checksum_with_payload", [condition, data, checksum, algo])
                if condition == "condition"
                    && data == "data"
                    && checksum == "checksum"
                    && algo == "algo" =>
            {
                let packet = get_packet_in_extern(&mut bridge, architecture)?;
                let context =
                    eval_verify_checksum(&mut bridge, context, architecture, Some(&packet))?;
                (
                    context,
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("update_checksum", [condition, data, checksum, algo])
                if condition == "condition"
                    && data == "data"
                    && checksum == "checksum"
                    && algo == "algo" =>
            {
                let context = eval_update_checksum(&mut bridge, context, architecture, None)?;
                (
                    context,
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("update_checksum_with_payload", [condition, data, checksum, algo])
                if condition == "condition"
                    && data == "data"
                    && checksum == "checksum"
                    && algo == "algo" =>
            {
                let packet = get_packet_in_extern(&mut bridge, architecture)?;
                let context =
                    eval_update_checksum(&mut bridge, context, architecture, Some(&packet))?;
                (
                    context,
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("clone_preserving_field_list", [kind, session, index])
                if kind == "type" && session == "session" && index == "index" =>
            {
                let kind = bridge
                    .find_var(&cursor, context, "type")
                    .map_err(sim_extern_error)?;
                let (_, kind) = unpack_p4_enum(&kind).map_err(sim_extern_error)?;
                let kind = match kind.as_str() {
                    "I2E" => CloneKind::I2E,
                    "E2E" => CloneKind::E2E,
                    _ => return Err(extern_error("invalid CloneType")),
                };
                let session = local_fixed_i32(&mut bridge, context, "session")?;
                let index = local_fixed_i32(&mut bridge, context, "index")?;
                let architecture =
                    update_architecture_state(&mut bridge, architecture.clone(), |state| {
                        state.clone_action = Some(CloneAction {
                            kind,
                            session,
                            index,
                        });
                    })
                    .map_err(sim_extern_error)?;
                (
                    context.clone(),
                    architecture,
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("resubmit_preserving_field_list", [index]) if index == "index" => {
                let index = local_fixed_i32(&mut bridge, context, "index")?;
                let architecture =
                    update_architecture_state(&mut bridge, architecture.clone(), |state| {
                        state.resubmit = Some(index)
                    })
                    .map_err(sim_extern_error)?;
                (
                    context.clone(),
                    architecture,
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("recirculate_preserving_field_list", [index]) if index == "index" => {
                let index = local_fixed_i32(&mut bridge, context, "index")?;
                let architecture =
                    update_architecture_state(&mut bridge, architecture.clone(), |state| {
                        state.recirculate = Some(index)
                    })
                    .map_err(sim_extern_error)?;
                (
                    context.clone(),
                    architecture,
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("log_msg", [message]) if message == "msg" => {
                let message = bridge
                    .find_var(&cursor, context, "msg")
                    .map_err(sim_extern_error)?;
                println!("{}", unpack_p4_string(&message).map_err(sim_extern_error)?);
                (
                    context.clone(),
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            ("log_msg", [message, data]) if message == "msg" && data == "data" => {
                let message = bridge
                    .find_var(&cursor, context, "msg")
                    .map_err(sim_extern_error)?;
                let data = bridge
                    .find_var(&cursor, context, "data")
                    .map_err(sim_extern_error)?;
                let message = unpack_p4_string(&message).map_err(sim_extern_error)?;
                let data = unpack_p4_tuple(&data).map_err(sim_extern_error)?;
                println!(
                    "{}",
                    format_log_message(&message, &data).map_err(sim_extern_error)?
                );
                (
                    context.clone(),
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                )
            }
            _ => {
                return Err(extern_error(format!(
                    "unsupported extern function call: {name}({})",
                    parameters.join(", ")
                )));
            }
        };
        Ok(vec![context, architecture, result])
    }

    fn eval_extern_method(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        let [context, architecture, object_id, name, parameters] = values else {
            return Err(extern_error("unexpected extern method call shape"));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let parameters = text_list(parameters)?;
        let mut bridge = Spec::new(spec);
        let state = bridge
            .find_object_state(architecture, object_id)
            .map_err(sim_extern_error)?;
        let object = V1Object::from_external(get::external(&state).map_err(value_extern_error)?)
            .map_err(sim_extern_error)?;
        let (object, context, result) = self.object_method(
            &mut bridge,
            object,
            context,
            architecture,
            name,
            &parameters,
        )?;
        let state = external_value(
            "objectState",
            object.to_external().map_err(sim_extern_error)?,
        );
        let architecture = bridge
            .update_object_state(architecture, object_id, &state)
            .map_err(sim_extern_error)?;
        Ok(vec![context, architecture, result])
    }

    fn object_method(
        &mut self,
        spec: &mut Spec<'_>,
        object: V1Object,
        context: &ValueRef,
        architecture: &ValueRef,
        name: &str,
        parameters: &[String],
    ) -> Result<(V1Object, ValueRef, ValueRef), ExternError> {
        let cursor = local_cursor().map_err(sim_extern_error)?;
        match object {
            V1Object::PacketIn(mut packet) => {
                let (context, result) = match (name, parameters) {
                    ("extract", [header]) if header == "hdr" => {
                        packet_extract(spec, &mut packet, context, architecture, false)?
                    }
                    ("extract", [header, size])
                        if header == "variableSizeHeader" && size == "variableFieldSizeInBits" =>
                    {
                        packet_extract(spec, &mut packet, context, architecture, true)?
                    }
                    ("lookahead", []) => packet_lookahead(spec, &packet, context)?,
                    ("advance", [size]) if size == "sizeInBits" => {
                        packet_advance(spec, &mut packet, context)?
                    }
                    ("length", []) => {
                        let length =
                            pack_p4_fixed_bit(BigInt::from(32), BigInt::from(packet.len_bytes()))
                                .map_err(sim_extern_error)?;
                        (
                            context.clone(),
                            return_result(Some(length)).map_err(sim_extern_error)?,
                        )
                    }
                    _ => return Err(unsupported_method("packet_in", name, parameters)),
                };
                Ok((V1Object::PacketIn(packet), context, result))
            }
            V1Object::PacketOut(mut packet) => {
                if !matches!((name, parameters), ("emit", [header]) if header == "hdr") {
                    return Err(unsupported_method("packet_out", name, parameters));
                }
                let header = spec
                    .find_var(&cursor, context, "hdr")
                    .map_err(sim_extern_error)?;
                packet.emit(
                    &spec
                        .write_bits_from_value(&header)
                        .map_err(sim_extern_error)?,
                );
                Ok((
                    V1Object::PacketOut(packet),
                    context.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                ))
            }
            V1Object::Counter(mut counter) => {
                if !matches!((name, parameters), ("count", [index]) if index == "index") {
                    return Err(unsupported_method("counter", name, parameters));
                }
                let index = fixed_usize(
                    &spec
                        .find_var(&cursor, context, "index")
                        .map_err(sim_extern_error)?,
                )?;
                count_counter(&mut counter, index, packet_len(spec, architecture)?)?;
                Ok((
                    V1Object::Counter(counter),
                    context.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                ))
            }
            V1Object::Register(mut register) => {
                let index = fixed_usize(
                    &spec
                        .find_var(&cursor, context, "index")
                        .map_err(sim_extern_error)?,
                )?;
                let (context, result) = match (name, parameters) {
                    ("read", [result, index_name])
                        if result == "result" && index_name == "index" =>
                    {
                        let value = match register.read(index) {
                            Some(value) => value.clone(),
                            None => spec
                                .default_value(register.typ())
                                .map_err(sim_extern_error)?,
                        };
                        let context = spec
                            .lvalue_write(
                                &cursor,
                                context,
                                architecture,
                                &prefixed_name("result").map_err(sim_extern_error)?,
                                &value,
                            )
                            .map_err(sim_extern_error)?;
                        (context, return_result(None).map_err(sim_extern_error)?)
                    }
                    ("write", [index_name, value_name])
                        if index_name == "index" && value_name == "value" =>
                    {
                        let value = spec
                            .find_var(&cursor, context, "value")
                            .map_err(sim_extern_error)?;
                        register.write(index, value);
                        (
                            context.clone(),
                            return_result(None).map_err(sim_extern_error)?,
                        )
                    }
                    _ => return Err(unsupported_method("register", name, parameters)),
                };
                Ok((V1Object::Register(register), context, result))
            }
            V1Object::DirectCounter(mut counter) => {
                if !matches!((name, parameters), ("count", [])) {
                    return Err(unsupported_method("direct_counter", name, parameters));
                }
                count_direct_counter(&mut counter, packet_len(spec, architecture)?);
                Ok((
                    V1Object::DirectCounter(counter),
                    context.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                ))
            }
            V1Object::DirectMeter(meter) => {
                if !matches!((name, parameters), ("read", [result]) if result == "result") {
                    return Err(unsupported_method("direct_meter", name, parameters));
                }
                let typ = required_type(spec, &cursor, context, "T")?;
                let substituted = spec
                    .substitute_type(&cursor, context, &typ)
                    .map_err(sim_extern_error)?;
                let width = spec
                    .sizeof_max_bits(&substituted)
                    .map_err(sim_extern_error)?;
                let value = pack_p4_fixed_bit(width, BigInt::zero()).map_err(sim_extern_error)?;
                let context = spec
                    .lvalue_write(
                        &cursor,
                        context,
                        architecture,
                        &prefixed_name("result").map_err(sim_extern_error)?,
                        &value,
                    )
                    .map_err(sim_extern_error)?;
                Ok((
                    V1Object::DirectMeter(meter),
                    context,
                    return_result(None).map_err(sim_extern_error)?,
                ))
            }
        }
    }
}

impl Default for V1Model {
    fn default() -> Self {
        Self::new()
    }
}

impl Extern for V1Model {
    fn eval_rel(
        &mut self,
        spec: &mut dyn SpecCall,
        name: &str,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        match name {
            "ExternFunctionCall_eval_lctk" => self.eval_compile_time_function(spec, values),
            "ExternFunctionCall_eval" => self.eval_extern_function(spec, values),
            "ExternMethodCall_eval" => self.eval_extern_method(spec, values),
            _ => Err(extern_error(format!(
                "unimplemented extern relation: {name}"
            ))),
        }
    }

    fn eval_func(
        &mut self,
        spec: &mut dyn SpecCall,
        name: &str,
        _type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        match name {
            "init_objectState" => self.eval_extern_init(spec, values),
            "init_archState" => Ok(external_value(
                "archState",
                architecture_to_external(&ArchitectureState::default()),
            )),
            _ => Err(extern_error(format!(
                "unimplemented extern function: {name}"
            ))),
        }
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

#[derive(Clone)]
struct ScheduledPacket {
    context: ValueRef,
    packet: PacketIn,
    ingress: bool,
}

struct PipelineState {
    context: ValueRef,
    architecture: ValueRef,
    queue: VecDeque<ScheduledPacket>,
    transmitted: Vec<Tx>,
}

impl PipelineState {
    fn drive(
        spec: &mut Spec<'_>,
        context: ValueRef,
        architecture: ValueRef,
        rx: Rx,
    ) -> Result<(ValueRef, ValueRef, Vec<Tx>), SimError> {
        let packet = PacketIn::new(&rx.packet)?;
        let packet_state = object_state_value(V1Object::PacketIn(packet))?;
        let (context, architecture) =
            spec.v1model_init_packet(false, &context, &architecture, &packet_state)?;
        let output_state = object_state_value(V1Object::PacketOut(PacketOut::new()))?;
        let (context, architecture) =
            spec.v1model_init_packet(true, &context, &architecture, &output_state)?;
        let context = spec.v1model_init_globals(&context, &architecture, rx.port)?;
        let mut state = Self {
            context,
            architecture,
            queue: VecDeque::new(),
            transmitted: Vec::new(),
        };
        state.run_pre(spec)?;
        state.schedule_current(spec, true, true)?;
        while let Some(packet) = state.queue.pop_front() {
            state.reset_actions(spec)?;
            state.context = packet.context;
            state.architecture = put_object(
                spec,
                &state.architecture,
                &["packet_in"],
                V1Object::PacketIn(packet.packet),
            )?;
            if packet.ingress {
                state.process_ingress(spec)?;
            } else {
                state.process_egress(spec)?;
            }
        }
        Ok((state.context, state.architecture, state.transmitted))
    }

    fn reset_actions(&mut self, spec: &mut Spec<'_>) -> Result<(), SimError> {
        self.architecture = update_architecture_state(
            spec,
            self.architecture.clone(),
            ArchitectureState::reset_actions,
        )?;
        Ok(())
    }

    fn run_pre(&mut self, spec: &mut Spec<'_>) -> Result<(), SimError> {
        self.reset_actions(spec)?;
        let mut packet = get_packet_in_sim(spec, &self.architecture)?;
        packet.reset();
        self.architecture = put_object(
            spec,
            &self.architecture,
            &["packet_in"],
            V1Object::PacketIn(packet),
        )?;
        let (context, architecture, parser_result) =
            spec.v1model_stage("parser", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        self.record_parser_error(spec, &parser_result)?;
        let (context, architecture, _) =
            spec.v1model_stage("verify", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        Ok(())
    }

    fn process_ingress(&mut self, spec: &mut Spec<'_>) -> Result<(), SimError> {
        let (context, architecture, _) =
            spec.v1model_stage("ingress", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        let actions = get_architecture_state(spec, &self.architecture)?;
        self.schedule_clone(spec, &actions)?;
        if self.schedule_resubmit(spec, &actions)? {
            return Ok(());
        }
        let group = self.read_fixed(spec, "standard_metadata", "mcast_grp")?;
        let group = group
            .to_i32()
            .ok_or_else(|| SimError::message("multicast group does not fit i32"))?;
        if group != 0 {
            self.schedule_multicast(spec, &actions, group)?;
        } else if !self.is_dropped(spec)? {
            self.schedule_current(spec, false, false)?;
        }
        Ok(())
    }

    fn process_egress(&mut self, spec: &mut Spec<'_>) -> Result<(), SimError> {
        let egress_spec = self.read_value(spec, "standard_metadata", "egress_spec")?;
        self.context = self.write_value(spec, "standard_metadata", "egress_port", &egress_spec)?;
        let (context, architecture, _) =
            spec.v1model_stage("egress", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        let actions = get_architecture_state(spec, &self.architecture)?;
        self.schedule_clone(spec, &actions)?;
        if self.is_dropped(spec)? {
            return Ok(());
        }
        if self.schedule_recirculate(spec, &actions)? {
            return Ok(());
        }
        self.run_post(spec)
    }

    fn run_post(&mut self, spec: &mut Spec<'_>) -> Result<(), SimError> {
        let (context, architecture, _) =
            spec.v1model_stage("check", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        self.architecture = put_object(
            spec,
            &self.architecture,
            &["packet_out"],
            V1Object::PacketOut(PacketOut::new()),
        )?;
        let (context, architecture, _) =
            spec.v1model_stage("deparse", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        let port = self
            .read_fixed(spec, "standard_metadata", "egress_spec")?
            .to_i32()
            .ok_or_else(|| SimError::message("egress_spec does not fit i32"))?;
        self.transmitted
            .push(Tx::new(port, compose_packet(spec, &self.architecture)?));
        Ok(())
    }

    fn record_parser_error(
        &mut self,
        spec: &mut Spec<'_>,
        result: &ValueRef,
    ) -> Result<(), SimError> {
        if !super::core::is_reject(result)? {
            return Ok(());
        }
        let values = get::case(result).map_err(value_error)?.args();
        let [error] = values.as_slice() else {
            return Err(SimError::message("parser reject result has invalid shape"));
        };
        self.context = self.write_value(spec, "standard_metadata", "parser_error", error)?;
        Ok(())
    }

    fn read_value(
        &mut self,
        spec: &mut Spec<'_>,
        base: &str,
        member: &str,
    ) -> Result<ValueRef, SimError> {
        spec.lvalue_read(
            &global_cursor()?,
            &self.context,
            &self.architecture,
            &storage_reference(&[base, member])?,
        )
    }

    fn read_fixed(
        &mut self,
        spec: &mut Spec<'_>,
        base: &str,
        member: &str,
    ) -> Result<BigInt, SimError> {
        unpack_p4_fixed_bit(&self.read_value(spec, base, member)?).map(|(_, value)| value)
    }

    fn write_value(
        &mut self,
        spec: &mut Spec<'_>,
        base: &str,
        member: &str,
        value: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        spec.lvalue_write(
            &global_cursor()?,
            &self.context,
            &self.architecture,
            &storage_reference(&[base, member])?,
            value,
        )
    }

    fn is_dropped(&mut self, spec: &mut Spec<'_>) -> Result<bool, SimError> {
        let value = self.read_value(spec, "standard_metadata", "egress_spec")?;
        let (width, value) = unpack_p4_fixed_bit(&value)?;
        Ok(width == BigInt::from(9) && value == BigInt::from(511))
    }

    fn schedule_current(
        &mut self,
        spec: &mut Spec<'_>,
        ingress: bool,
        front: bool,
    ) -> Result<(), SimError> {
        let packet = ScheduledPacket {
            context: self.context.clone(),
            packet: get_packet_in_sim(spec, &self.architecture)?,
            ingress,
        };
        if front {
            self.queue.push_front(packet);
        } else {
            self.queue.push_back(packet);
        }
        Ok(())
    }

    fn prepare_preserved(&mut self, spec: &mut Spec<'_>, index: i32) -> Result<(), SimError> {
        let index = pack_p4_fixed_bit(BigInt::from(8), BigInt::from(index))?;
        self.context =
            spec.v1model_setup_preserved_metadata(&self.context, &self.architecture, &index)?;
        Ok(())
    }

    fn set_metadata_fixed(
        &mut self,
        spec: &mut Spec<'_>,
        member: &str,
        width: i32,
        value: i32,
    ) -> Result<(), SimError> {
        let value = pack_p4_fixed_bit(BigInt::from(width), BigInt::from(value))?;
        self.context = self.write_value(spec, "standard_metadata", member, &value)?;
        Ok(())
    }

    fn schedule_clone(
        &mut self,
        spec: &mut Spec<'_>,
        actions: &ArchitectureState,
    ) -> Result<bool, SimError> {
        let Some(action) = actions.clone_action else {
            return Ok(false);
        };
        let Some(port) = actions.mirrors.get(&action.session).copied() else {
            return Ok(false);
        };
        let original_context = self.context.clone();
        self.prepare_preserved(spec, action.index)?;
        self.set_metadata_fixed(
            spec,
            "instance_type",
            32,
            match action.kind {
                CloneKind::I2E => 1,
                CloneKind::E2E => 2,
            },
        )?;
        self.set_metadata_fixed(spec, "egress_spec", 9, port)?;
        if action.kind == CloneKind::I2E {
            self.run_pre(spec)?;
        }
        self.schedule_current(spec, false, false)?;
        self.context = original_context;
        Ok(true)
    }

    fn schedule_resubmit(
        &mut self,
        spec: &mut Spec<'_>,
        actions: &ArchitectureState,
    ) -> Result<bool, SimError> {
        let Some(index) = actions.resubmit else {
            return Ok(false);
        };
        let original_context = self.context.clone();
        self.prepare_preserved(spec, index)?;
        self.set_metadata_fixed(spec, "instance_type", 32, 6)?;
        self.run_pre(spec)?;
        self.schedule_current(spec, true, true)?;
        self.context = original_context;
        Ok(true)
    }

    fn schedule_recirculate(
        &mut self,
        spec: &mut Spec<'_>,
        actions: &ArchitectureState,
    ) -> Result<bool, SimError> {
        let Some(index) = actions.recirculate else {
            return Ok(false);
        };
        let original_context = self.context.clone();
        self.prepare_preserved(spec, index)?;
        self.set_metadata_fixed(spec, "instance_type", 32, 4)?;
        let (context, architecture, _) =
            spec.v1model_stage("check", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        self.architecture = put_object(
            spec,
            &self.architecture,
            &["packet_out"],
            V1Object::PacketOut(PacketOut::new()),
        )?;
        let (context, architecture, _) =
            spec.v1model_stage("deparse", &self.context, &self.architecture)?;
        self.context = context;
        self.architecture = architecture;
        let packet = PacketIn::new(&compose_packet(spec, &self.architecture)?)?;
        self.architecture = put_object(
            spec,
            &self.architecture,
            &["packet_in"],
            V1Object::PacketIn(packet),
        )?;
        self.run_pre(spec)?;
        self.schedule_current(spec, true, true)?;
        self.context = original_context;
        Ok(true)
    }

    fn schedule_multicast(
        &mut self,
        spec: &mut Spec<'_>,
        actions: &ArchitectureState,
        group: i32,
    ) -> Result<bool, SimError> {
        let replicas = actions.multicast.replicas(group);
        for (port, rid) in replicas.iter().copied() {
            self.set_metadata_fixed(spec, "egress_rid", 16, rid)?;
            self.set_metadata_fixed(spec, "egress_spec", 9, port)?;
            self.set_metadata_fixed(spec, "instance_type", 32, 5)?;
            self.schedule_current(spec, false, false)?;
        }
        Ok(!replicas.is_empty())
    }
}

impl Architecture for V1Model {
    fn name() -> &'static str {
        "v1model"
    }

    fn init(spec: &mut dyn SpecCall, program: &ValueRef) -> Result<(ValueRef, ValueRef), SimError> {
        Spec::new(spec).v1model_init(program)
    }

    fn drive(
        spec: &mut dyn SpecCall,
        context: ValueRef,
        architecture: ValueRef,
        rx: Rx,
    ) -> Result<(ValueRef, ValueRef, Vec<Tx>), SimError> {
        PipelineState::drive(&mut Spec::new(spec), context, architecture, rx)
    }

    fn transform_stf(statement: StfStmt) -> StfStmt {
        transform_stf(statement)
    }

    fn add_mirror_session(
        spec: &mut dyn SpecCall,
        architecture: ValueRef,
        session: i32,
        port: i32,
    ) -> Result<ValueRef, SimError> {
        let mut bridge = Spec::new(spec);
        update_architecture_state(&mut bridge, architecture, |state| {
            state.mirrors.insert(session, port);
        })
    }

    fn mc_group_create(
        spec: &mut dyn SpecCall,
        architecture: ValueRef,
        group: i32,
    ) -> Result<ValueRef, SimError> {
        let mut bridge = Spec::new(spec);
        update_architecture_state(&mut bridge, architecture, |state| {
            state.multicast.create_group(group);
        })
    }

    fn mc_node_create(
        spec: &mut dyn SpecCall,
        architecture: ValueRef,
        rid: i32,
        ports: Vec<i32>,
    ) -> Result<ValueRef, SimError> {
        let mut bridge = Spec::new(spec);
        update_architecture_state(&mut bridge, architecture, |state| {
            state.multicast.create_node(rid, ports);
        })
    }

    fn mc_node_associate(
        spec: &mut dyn SpecCall,
        architecture: ValueRef,
        group: i32,
        handle: i32,
    ) -> Result<ValueRef, SimError> {
        let mut bridge = Spec::new(spec);
        update_architecture_state(&mut bridge, architecture, |state| {
            state.multicast.associate(group, handle);
        })
    }
}

pub fn transform_stf(statement: StfStmt) -> StfStmt {
    match statement {
        StfStmt::Add {
            name,
            priority,
            matches,
            action,
            id,
        } => StfStmt::Add {
            name: transform_name(name),
            priority,
            matches: matches.into_iter().map(transform_match).collect(),
            action: transform_action(action),
            id,
        },
        StfStmt::SetDefault { name, action } => StfStmt::SetDefault {
            name: transform_name(name),
            action: transform_action(action),
        },
        statement => statement,
    }
}

fn transform_name(name: String) -> String {
    let name = rewrite_first_component(name, &["ingress", "preqos"], "main.ig");
    rewrite_first_component(name, &["egress", "postqos", "c3"], "main.eg")
}

fn rewrite_first_component(name: String, patterns: &[&str], replacement: &str) -> String {
    let mut parts = name.split('.');
    let Some(first) = parts.next() else {
        return name;
    };
    let lowercase = first.to_ascii_lowercase();
    if !patterns
        .iter()
        .any(|pattern| lowercase.contains(&pattern.to_ascii_lowercase()))
    {
        return name;
    }
    std::iter::once(replacement)
        .chain(parts)
        .collect::<Vec<_>>()
        .join(".")
}

fn transform_match(mut entry: StfMatch) -> StfMatch {
    entry.name = entry.name.replace("$valid$", "isValid()");
    entry
}

fn transform_action(mut action: StfAction) -> StfAction {
    action.name = transform_name(action.name);
    action
}

fn object_state_value(object: V1Object) -> Result<ValueRef, SimError> {
    Ok(external_value("objectState", object.to_external()?))
}

fn get_object(
    spec: &mut Spec<'_>,
    architecture: &ValueRef,
    names: &[&str],
) -> Result<V1Object, SimError> {
    let state = spec.find_object_state(architecture, &super::core::encode_object_id(names))?;
    V1Object::from_external(get::external(&state).map_err(value_error)?)
}

fn put_object(
    spec: &mut Spec<'_>,
    architecture: &ValueRef,
    names: &[&str],
    object: V1Object,
) -> Result<ValueRef, SimError> {
    spec.update_object_state(
        architecture,
        &super::core::encode_object_id(names),
        &object_state_value(object)?,
    )
}

fn get_packet_in_sim(spec: &mut Spec<'_>, architecture: &ValueRef) -> Result<PacketIn, SimError> {
    match get_object(spec, architecture, &["packet_in"])? {
        V1Object::PacketIn(packet) => Ok(packet),
        _ => Err(SimError::message("packet_in object not found")),
    }
}

fn compose_packet(spec: &mut Spec<'_>, architecture: &ValueRef) -> Result<String, SimError> {
    let input = get_packet_in_sim(spec, architecture)?;
    match get_object(spec, architecture, &["packet_out"])? {
        V1Object::PacketOut(output) => Ok(output.packet_hex(&input)),
        _ => Err(SimError::message("packet_out object not found")),
    }
}

fn count_counter(counter: &mut Counter, index: usize, bytes: usize) -> Result<(), ExternError> {
    match counter {
        Counter::Packets(values) => {
            if let Some(value) = values.get_mut(index) {
                *value += BigInt::one();
            }
        }
        Counter::Bytes(values) => {
            if let Some(value) = values.get_mut(index) {
                *value += BigInt::from(bytes);
            }
        }
        Counter::PacketsAndBytes(values) => {
            if let Some((packets, byte_count)) = values.get_mut(index) {
                *packets += BigInt::one();
                *byte_count += BigInt::from(bytes);
            }
        }
    }
    Ok(())
}

fn count_direct_counter(counter: &mut DirectCounter, bytes: usize) {
    match counter {
        DirectCounter::Packets(value) => *value += BigInt::one(),
        DirectCounter::Bytes(value) => *value += BigInt::from(bytes),
        DirectCounter::PacketsAndBytes(packets, byte_count) => {
            *packets += BigInt::one();
            *byte_count += BigInt::from(bytes);
        }
    }
}

fn packet_len(spec: &mut Spec<'_>, architecture: &ValueRef) -> Result<usize, ExternError> {
    let state = spec
        .find_object_state(architecture, &super::core::encode_object_id(&["packet_in"]))
        .map_err(sim_extern_error)?;
    match V1Object::from_external(get::external(&state).map_err(value_extern_error)?)
        .map_err(sim_extern_error)?
    {
        V1Object::PacketIn(packet) => Ok(packet.len_bytes()),
        _ => Err(extern_error("packet_in object not found")),
    }
}

fn get_packet_in_extern(
    spec: &mut Spec<'_>,
    architecture: &ValueRef,
) -> Result<PacketIn, ExternError> {
    let state = spec
        .find_object_state(architecture, &super::core::encode_object_id(&["packet_in"]))
        .map_err(sim_extern_error)?;
    match V1Object::from_external(get::external(&state).map_err(value_extern_error)?)
        .map_err(sim_extern_error)?
    {
        V1Object::PacketIn(packet) => Ok(packet),
        _ => Err(extern_error("packet_in object not found")),
    }
}

fn local_var(spec: &mut Spec<'_>, context: &ValueRef, name: &str) -> Result<ValueRef, ExternError> {
    spec.find_var(&local_cursor().map_err(sim_extern_error)?, context, name)
        .map_err(sim_extern_error)
}

fn local_fixed_i32(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    name: &str,
) -> Result<i32, ExternError> {
    unpack_p4_fixed_bit(&local_var(spec, context, name)?)
        .map_err(sim_extern_error)?
        .1
        .to_i32()
        .ok_or_else(|| extern_error(format!("{name} does not fit i32")))
}

fn write_local_member(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    base: &str,
    member: &str,
    value: &ValueRef,
) -> Result<ValueRef, ExternError> {
    spec.lvalue_write(
        &local_cursor().map_err(sim_extern_error)?,
        context,
        architecture,
        &storage_reference(&[base, member]).map_err(sim_extern_error)?,
        value,
    )
    .map_err(sim_extern_error)
}

fn write_global_member(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    base: &str,
    member: &str,
    value: &ValueRef,
) -> Result<ValueRef, ExternError> {
    spec.lvalue_write(
        &global_cursor().map_err(sim_extern_error)?,
        context,
        architecture,
        &storage_reference(&[base, member]).map_err(sim_extern_error)?,
        value,
    )
    .map_err(sim_extern_error)
}

fn write_local_var(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    name: &str,
    value: &ValueRef,
) -> Result<ValueRef, ExternError> {
    spec.lvalue_write(
        &local_cursor().map_err(sim_extern_error)?,
        context,
        architecture,
        &prefixed_name(name).map_err(sim_extern_error)?,
        value,
    )
    .map_err(sim_extern_error)
}

fn checksum_inputs(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    payload: Option<&PacketIn>,
) -> Result<(String, Vec<ValueRef>), ExternError> {
    let data = local_var(spec, context, "data")?;
    let mut values = unpack_p4_tuple(&data).map_err(sim_extern_error)?;
    if let Some(packet) = payload {
        for byte in packet.payload_bytes() {
            values.push(pack_p4_fixed_bit(BigInt::from(8), byte).map_err(sim_extern_error)?);
        }
    }
    let algorithm = local_var(spec, context, "algo")?;
    let (enum_name, algorithm) = unpack_p4_enum(&algorithm).map_err(sim_extern_error)?;
    if enum_name != "HashAlgorithm" {
        return Err(extern_error("invalid HashAlgorithm"));
    }
    Ok((algorithm, values))
}

fn eval_hash(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
) -> Result<ValueRef, ExternError> {
    let base = unpack_p4_fixed_bit(&local_var(spec, context, "base")?)
        .map_err(sim_extern_error)?
        .1;
    let maximum = unpack_p4_fixed_bit(&local_var(spec, context, "max")?)
        .map_err(sim_extern_error)?
        .1;
    let (algorithm, values) = checksum_inputs(spec, context, None)?;
    let checksum =
        compute_checksum(&algorithm, &values, &BigInt::zero()).map_err(sim_extern_error)?;
    let result = if maximum.is_zero() {
        base
    } else {
        &base + checksum % (&maximum - &base)
    };
    let typ = required_type(
        spec,
        &local_cursor().map_err(sim_extern_error)?,
        context,
        "O",
    )?;
    let result = pack_p4_arbitrary_int(result).map_err(sim_extern_error)?;
    let result = spec.cast(&typ, &result).map_err(sim_extern_error)?;
    write_local_var(spec, context, architecture, "result", &result)
}

fn eval_verify_checksum(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    payload: Option<&PacketIn>,
) -> Result<ValueRef, ExternError> {
    if !unpack_p4_bool(&local_var(spec, context, "condition")?).map_err(sim_extern_error)? {
        return Ok(context.clone());
    }
    let expected = unpack_p4_fixed_bit(&local_var(spec, context, "checksum")?)
        .map_err(sim_extern_error)?
        .1;
    let (algorithm, values) = checksum_inputs(spec, context, payload)?;
    let actual =
        compute_checksum(&algorithm, &values, &BigInt::zero()).map_err(sim_extern_error)?;
    if expected == actual {
        Ok(context.clone())
    } else {
        write_global_member(
            spec,
            context,
            architecture,
            "standard_metadata",
            "checksum_error",
            &pack_p4_fixed_bit(BigInt::one(), BigInt::one()).map_err(sim_extern_error)?,
        )
    }
}

fn eval_update_checksum(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    payload: Option<&PacketIn>,
) -> Result<ValueRef, ExternError> {
    if !unpack_p4_bool(&local_var(spec, context, "condition")?).map_err(sim_extern_error)? {
        return Ok(context.clone());
    }
    let (algorithm, values) = checksum_inputs(spec, context, payload)?;
    let checksum =
        compute_checksum(&algorithm, &values, &BigInt::zero()).map_err(sim_extern_error)?;
    let typ = required_type(
        spec,
        &local_cursor().map_err(sim_extern_error)?,
        context,
        "O",
    )?;
    let checksum = pack_p4_arbitrary_int(checksum).map_err(sim_extern_error)?;
    let checksum = spec.cast(&typ, &checksum).map_err(sim_extern_error)?;
    write_local_var(spec, context, architecture, "checksum", &checksum)
}

fn architecture_to_external(state: &ArchitectureState) -> ExternalData {
    let pairs = |values: &BTreeMap<i32, i32>| {
        ExternalData::List(
            values
                .iter()
                .map(|(first, second)| {
                    ExternalData::Tuple(vec![
                        ExternalData::Int(i64::from(*first)),
                        ExternalData::Int(i64::from(*second)),
                    ])
                })
                .collect(),
        )
    };
    let groups = ExternalData::List(
        state
            .multicast
            .groups
            .iter()
            .map(|(group, handles)| {
                ExternalData::Tuple(vec![
                    ExternalData::Int(i64::from(*group)),
                    ExternalData::List(
                        handles
                            .iter()
                            .map(|handle| ExternalData::Int(i64::from(*handle)))
                            .collect(),
                    ),
                ])
            })
            .collect(),
    );
    let nodes = ExternalData::List(
        state
            .multicast
            .nodes
            .iter()
            .map(|(handle, replicas)| {
                ExternalData::Tuple(vec![
                    ExternalData::Int(i64::from(*handle)),
                    ExternalData::List(
                        replicas
                            .iter()
                            .map(|(port, rid)| {
                                ExternalData::Tuple(vec![
                                    ExternalData::Int(i64::from(*port)),
                                    ExternalData::Int(i64::from(*rid)),
                                ])
                            })
                            .collect(),
                    ),
                ])
            })
            .collect(),
    );
    let clone_action = match state.clone_action {
        Some(action) => ExternalData::Tuple(vec![
            ExternalData::String(
                match action.kind {
                    CloneKind::I2E => "i2e",
                    CloneKind::E2E => "e2e",
                }
                .to_owned(),
            ),
            ExternalData::Int(i64::from(action.session)),
            ExternalData::Int(i64::from(action.index)),
        ]),
        None => ExternalData::Null,
    };
    ExternalData::Assoc(vec![
        (
            "kind".to_owned(),
            ExternalData::String("v1model-architecture".to_owned()),
        ),
        ("mirrors".to_owned(), pairs(&state.mirrors)),
        (
            "multicast-next-handle".to_owned(),
            ExternalData::Int(i64::from(state.multicast.next_handle)),
        ),
        ("multicast-groups".to_owned(), groups),
        ("multicast-nodes".to_owned(), nodes),
        ("clone".to_owned(), clone_action),
        ("resubmit".to_owned(), encode_optional_i32(state.resubmit)),
        (
            "recirculate".to_owned(),
            encode_optional_i32(state.recirculate),
        ),
    ])
}

fn architecture_from_external(value: &ExternalData) -> Result<ArchitectureState, SimError> {
    let ExternalData::Assoc(fields) = value else {
        return Err(SimError::message("expected v1model architecture state"));
    };
    if string_field(fields, "kind")? != "v1model-architecture" {
        return Err(SimError::message("expected v1model architecture state"));
    }
    let mirrors = decode_map(field(fields, "mirrors")?)?;
    let next_handle = decode_i32(field(fields, "multicast-next-handle")?)?;
    let groups = decode_map_lists(field(fields, "multicast-groups")?)?;
    let nodes = decode_map_pairs(field(fields, "multicast-nodes")?)?;
    let clone_action = match field(fields, "clone")? {
        ExternalData::Null => None,
        ExternalData::Tuple(values) => {
            let [kind, session, index] = values.as_slice() else {
                return Err(SimError::message("clone action must have three fields"));
            };
            let ExternalData::String(kind) = kind else {
                return Err(SimError::message("clone kind must be a string"));
            };
            Some(CloneAction {
                kind: match kind.as_str() {
                    "i2e" => CloneKind::I2E,
                    "e2e" => CloneKind::E2E,
                    _ => return Err(SimError::message("invalid clone kind")),
                },
                session: decode_i32(session)?,
                index: decode_i32(index)?,
            })
        }
        _ => return Err(SimError::message("invalid clone action")),
    };
    Ok(ArchitectureState {
        mirrors,
        multicast: V1Multicast {
            next_handle,
            groups,
            nodes,
        },
        clone_action,
        resubmit: decode_optional_i32(field(fields, "resubmit")?)?,
        recirculate: decode_optional_i32(field(fields, "recirculate")?)?,
    })
}

fn get_architecture_state(
    spec: &mut Spec<'_>,
    architecture: &ValueRef,
) -> Result<ArchitectureState, SimError> {
    let state = spec.find_arch_state(architecture)?;
    architecture_from_external(get::external(&state).map_err(value_error)?)
}

fn update_architecture_state(
    spec: &mut Spec<'_>,
    architecture: ValueRef,
    update: impl FnOnce(&mut ArchitectureState),
) -> Result<ValueRef, SimError> {
    let mut state = get_architecture_state(spec, &architecture)?;
    update(&mut state);
    spec.update_arch_state(
        &architecture,
        &external_value("archState", architecture_to_external(&state)),
    )
}

fn encode_optional_i32(value: Option<i32>) -> ExternalData {
    value
        .map(|value| ExternalData::Int(i64::from(value)))
        .unwrap_or(ExternalData::Null)
}

fn decode_optional_i32(value: &ExternalData) -> Result<Option<i32>, SimError> {
    match value {
        ExternalData::Null => Ok(None),
        value => decode_i32(value).map(Some),
    }
}

fn decode_i32(value: &ExternalData) -> Result<i32, SimError> {
    let ExternalData::Int(value) = value else {
        return Err(SimError::message("architecture value must be an integer"));
    };
    i32::try_from(*value).map_err(|_| SimError::message("architecture value does not fit i32"))
}

fn decode_map(value: &ExternalData) -> Result<BTreeMap<i32, i32>, SimError> {
    let ExternalData::List(values) = value else {
        return Err(SimError::message("architecture map must be a list"));
    };
    values
        .iter()
        .map(|value| {
            let ExternalData::Tuple(values) = value else {
                return Err(SimError::message("architecture map entry must be a tuple"));
            };
            let [key, value] = values.as_slice() else {
                return Err(SimError::message(
                    "architecture map entry must have two values",
                ));
            };
            Ok((decode_i32(key)?, decode_i32(value)?))
        })
        .collect()
}

fn decode_map_lists(value: &ExternalData) -> Result<BTreeMap<i32, Vec<i32>>, SimError> {
    let ExternalData::List(values) = value else {
        return Err(SimError::message("architecture map must be a list"));
    };
    values
        .iter()
        .map(|value| {
            let ExternalData::Tuple(values) = value else {
                return Err(SimError::message("architecture map entry must be a tuple"));
            };
            let [key, values] = values.as_slice() else {
                return Err(SimError::message(
                    "architecture map entry must have two values",
                ));
            };
            let ExternalData::List(values) = values else {
                return Err(SimError::message("architecture map value must be a list"));
            };
            Ok((
                decode_i32(key)?,
                values.iter().map(decode_i32).collect::<Result<_, _>>()?,
            ))
        })
        .collect()
}

fn decode_map_pairs(value: &ExternalData) -> Result<BTreeMap<i32, Vec<(i32, i32)>>, SimError> {
    let ExternalData::List(values) = value else {
        return Err(SimError::message("architecture map must be a list"));
    };
    values
        .iter()
        .map(|value| {
            let ExternalData::Tuple(values) = value else {
                return Err(SimError::message("architecture map entry must be a tuple"));
            };
            let [key, pairs] = values.as_slice() else {
                return Err(SimError::message(
                    "architecture map entry must have two values",
                ));
            };
            let ExternalData::List(pairs) = pairs else {
                return Err(SimError::message("architecture map value must be a list"));
            };
            let pairs = pairs
                .iter()
                .map(|pair| {
                    let ExternalData::Tuple(pair) = pair else {
                        return Err(SimError::message("architecture pair must be a tuple"));
                    };
                    let [first, second] = pair.as_slice() else {
                        return Err(SimError::message("architecture pair must have two values"));
                    };
                    Ok((decode_i32(first)?, decode_i32(second)?))
                })
                .collect::<Result<_, SimError>>()?;
            Ok((decode_i32(key)?, pairs))
        })
        .collect()
}

fn value_error(error: impl ToString) -> SimError {
    SimError::message(error.to_string())
}

fn format_log_message(message: &str, arguments: &[ValueRef]) -> Result<String, SimError> {
    let mut output = String::with_capacity(message.len() + 64);
    let mut characters = message.chars().peekable();
    let mut arguments = arguments.iter();

    while let Some(character) = characters.next() {
        match (character, characters.peek()) {
            ('{', Some('{')) => {
                characters.next();
                output.push('{');
            }
            ('}', Some('}')) => {
                characters.next();
                output.push('}');
            }
            ('{', Some('}')) => {
                characters.next();
                let argument = arguments.next().ok_or_else(|| {
                    SimError::message("not enough arguments for format string in log_msg")
                })?;
                output.push_str(&format_runtime_value(argument, 0));
            }
            _ => output.push(character),
        }
    }

    if arguments.next().is_some() {
        return Err(SimError::message(
            "too many arguments for format string in log_msg",
        ));
    }
    Ok(output)
}

fn format_runtime_value(value: &ValueRef, level: usize) -> String {
    match &value.kind {
        ValueKind::BoolV(value) => value.to_string(),
        ValueKind::NumV(crate::lang::xl::num::T::Nat(value)) => value.to_string(),
        ValueKind::NumV(crate::lang::xl::num::T::Int(value)) => {
            let sign = if value.is_negative() { '-' } else { '+' };
            format!("{sign}{}", value.abs())
        }
        ValueKind::TextV(value) => value.chars().flat_map(char::escape_default).collect(),
        ValueKind::StructV(fields) if fields.is_empty() => "{}".to_owned(),
        ValueKind::StructV(fields) => {
            let field_indent = "  ".repeat(level + 1);
            let fields = fields
                .iter()
                .map(|(atom, value)| {
                    format!(
                        "{field_indent}{} {}",
                        atom.node.source_string(),
                        format_runtime_value(value, level + 1)
                    )
                })
                .collect::<Vec<_>>()
                .join(";\n");
            format!("{{\n{fields}\n{}}}", "  ".repeat(level))
        }
        ValueKind::CaseV(value_case) => value_case.render(
            |atom| atom.node.source_string(),
            |value| format_runtime_value(value, level + 1),
        ),
        ValueKind::TupleV(values) => format!(
            "({})",
            values
                .iter()
                .map(|value| format_runtime_value(value, level + 1))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ValueKind::OptV(Some(value)) => {
            format!("Some({})", format_runtime_value(value, level + 1))
        }
        ValueKind::OptV(None) => "None".to_owned(),
        ValueKind::ListV(values) if values.is_empty() => "[]".to_owned(),
        ValueKind::ListV(values) => {
            let value_indent = "  ".repeat(level + 1);
            let values = values
                .iter()
                .map(|value| format!("{value_indent}{}", format_runtime_value(value, level + 1)))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("[\n{values}\n{}]", "  ".repeat(level))
        }
        ValueKind::FuncV(id) => format!("${}", id.node),
        ValueKind::ExternV(_) => "extern".to_owned(),
    }
}

fn required_type(
    spec: &mut Spec<'_>,
    cursor: &ValueRef,
    context: &ValueRef,
    name: &str,
) -> Result<ValueRef, ExternError> {
    let typ = spec
        .find_type(cursor, context, name)
        .map_err(sim_extern_error)?;
    get::opt(&typ)
        .map_err(value_extern_error)?
        .cloned()
        .ok_or_else(|| extern_error(format!("find_type_e returned none for {name}")))
}

fn fixed_usize(value: &ValueRef) -> Result<usize, ExternError> {
    unpack_p4_fixed_bit(value)
        .map_err(sim_extern_error)?
        .1
        .to_usize()
        .ok_or_else(|| extern_error("fixed-width number does not fit usize"))
}

fn encode_bigint_list(values: &[BigInt]) -> ExternalData {
    ExternalData::List(values.iter().map(encode_bigint).collect())
}

fn decode_counter(fields: &[(String, ExternalData)]) -> Result<Counter, SimError> {
    let kind = string_field(fields, "counter-kind")?;
    let ExternalData::List(values) = field(fields, "values")? else {
        return Err(SimError::message("counter values must be a list"));
    };
    match kind {
        "packets" => Ok(Counter::Packets(
            values.iter().map(decode_bigint).collect::<Result<_, _>>()?,
        )),
        "bytes" => Ok(Counter::Bytes(
            values.iter().map(decode_bigint).collect::<Result<_, _>>()?,
        )),
        "packets-and-bytes" => Ok(Counter::PacketsAndBytes(
            values
                .iter()
                .map(|value| {
                    let ExternalData::Tuple(values) = value else {
                        return Err(SimError::message("counter pair must be a tuple"));
                    };
                    let [packets, bytes] = values.as_slice() else {
                        return Err(SimError::message("counter pair must have two values"));
                    };
                    Ok((decode_bigint(packets)?, decode_bigint(bytes)?))
                })
                .collect::<Result<_, SimError>>()?,
        )),
        _ => Err(SimError::message("invalid counter kind")),
    }
}

fn string_field<'a>(fields: &'a [(String, ExternalData)], name: &str) -> Result<&'a str, SimError> {
    let ExternalData::String(value) = field(fields, name)? else {
        return Err(SimError::message(format!("{name} must be a string")));
    };
    Ok(value)
}

fn field<'a>(
    fields: &'a [(String, ExternalData)],
    name: &str,
) -> Result<&'a ExternalData, SimError> {
    fields
        .iter()
        .find_map(|(field, value)| (field == name).then_some(value))
        .ok_or_else(|| SimError::message(format!("missing v1model state field `{name}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::source::Region, runtime::value::make};

    #[test]
    fn v1model_register_state_round_trips() {
        let typ = make::text("T".to_owned(), Region::none());
        let value = make::int(BigInt::from(5), Region::none());
        let object = V1Object::Register(Register::new(typ, 2, value));

        let external = object.to_external().unwrap();

        assert_eq!(V1Object::from_external(&external).unwrap(), object);
    }

    #[test]
    fn v1model_counters_include_packet_length() {
        let mut counter = Counter::packets_and_bytes(1);

        count_counter(&mut counter, 0, 12).unwrap();

        assert_eq!(
            counter,
            Counter::PacketsAndBytes(vec![(BigInt::one(), BigInt::from(12))])
        );
    }

    #[test]
    fn v1model_architecture_actions_round_trip_and_reset() {
        let mut state = ArchitectureState::default();
        state.mirrors.insert(4, 7);
        state.multicast.create_group(100);
        state.multicast.create_node(9, vec![2, 3]);
        state.multicast.associate(100, 0);
        state.clone_action = Some(CloneAction {
            kind: CloneKind::I2E,
            session: 4,
            index: 8,
        });

        let external = architecture_to_external(&state);
        let mut decoded = architecture_from_external(&external).unwrap();

        assert_eq!(decoded, state);
        decoded.reset_actions();
        assert_eq!(decoded.mirrors.get(&4), Some(&7));
        assert_eq!(decoded.multicast.replicas(100), vec![(2, 9), (3, 9)]);
        assert_eq!(decoded.clone_action, None);
    }

    #[test]
    fn v1model_stf_names_match_the_ocaml_transform() {
        let transformed = transform_stf(StfStmt::SetDefault {
            name: "ingress.tbl".to_owned(),
            action: StfAction {
                name: "egress.act".to_owned(),
                args: Vec::new(),
            },
        });
        let StfStmt::SetDefault { name, action } = transformed else {
            panic!("expected set-default statement");
        };
        assert_eq!(name, "main.ig.tbl");
        assert_eq!(action.name, "main.eg.act");
    }
}
