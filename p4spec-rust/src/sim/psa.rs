use std::collections::BTreeMap;

use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use crate::{
    domain::external_data::ExternalData,
    interface::{Extern, ExternError, SpecCall},
    lang::il::ast::Typ,
    runtime::value::{ValueRef, get},
    wire::{ocaml::lang::il::ValueCodec, runtime_value, sim_suite::StfStmt},
};

use super::{
    SimError,
    core::{
        PacketIn, PacketOut, extern_error, external_value, pack_p4_arbitrary_int, pack_p4_enum,
        pack_p4_fixed_bit, packet_advance, packet_extract, packet_lookahead, reject_result,
        return_result, sim_extern_error, text_list, unpack_p4_bool, unpack_p4_enum,
        unpack_p4_fixed_bit, unpack_p4_tuple, unsupported_method, value_extern_error,
    },
    hash::{bitwise_neg, compute_checksum},
    spec::{Spec, local_cursor},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Counter {
    Packets(Vec<BigInt>),
    Bytes(Vec<BigInt>),
    PacketsAndBytes(Vec<(BigInt, BigInt)>),
}

impl Counter {
    pub fn packets(size: usize) -> Self {
        Self::Packets(vec![BigInt::zero(); size])
    }

    pub fn bytes(size: usize) -> Self {
        Self::Bytes(vec![BigInt::zero(); size])
    }

    pub fn packets_and_bytes(size: usize) -> Self {
        Self::PacketsAndBytes(vec![(BigInt::zero(), BigInt::zero()); size])
    }

    pub fn count(&mut self, index: usize) -> Result<(), SimError> {
        let Self::Packets(values) = self else {
            return Err(SimError::message("only PACKETS PSA counters are supported"));
        };
        if let Some(value) = values.get_mut(index) {
            *value += BigInt::one();
        }
        Ok(())
    }

    pub fn packet_values(&self) -> Result<&[BigInt], SimError> {
        match self {
            Self::Packets(values) => Ok(values),
            _ => Err(SimError::message("counter does not count packets")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Register {
    typ: ValueRef,
    values: Vec<ValueRef>,
}

impl Register {
    pub fn new(typ: ValueRef, size: usize, initial: ValueRef) -> Self {
        Self {
            typ,
            values: vec![initial; size],
        }
    }

    pub fn typ(&self) -> &ValueRef {
        &self.typ
    }

    pub fn values(&self) -> &[ValueRef] {
        &self.values
    }

    pub fn read(&self, index: usize) -> Option<&ValueRef> {
        self.values.get(index)
    }

    pub fn write(&mut self, index: usize, value: ValueRef) {
        if let Some(target) = self.values.get_mut(index) {
            *target = value;
        }
    }

    pub fn reset(&mut self, value: ValueRef) {
        self.values.fill(value);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MulticastNode {
    pub port: i32,
    pub instance: i32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MulticastState {
    next_handle: i32,
    groups: BTreeMap<i32, Vec<i32>>,
    nodes: BTreeMap<i32, Vec<MulticastNode>>,
}

impl MulticastState {
    pub fn create_group(&mut self, group: i32) {
        self.groups.insert(group, Vec::new());
    }

    pub fn create_node(&mut self, instance: i32, ports: Vec<i32>) -> i32 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.nodes.insert(
            handle,
            ports
                .into_iter()
                .map(|port| MulticastNode { port, instance })
                .collect(),
        );
        handle
    }

    pub fn associate(&mut self, group: i32, handle: i32) {
        if let Some(handles) = self.groups.get_mut(&group) {
            handles.push(handle);
        }
    }

    pub fn replicas(&self, group: i32) -> Vec<(i32, i32)> {
        self.groups
            .get(&group)
            .into_iter()
            .flatten()
            .filter_map(|handle| self.nodes.get(handle))
            .flatten()
            .map(|node| (node.port, node.instance))
            .collect()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchitectureState {
    pub mirrors: BTreeMap<i32, i32>,
    pub multicast: MulticastState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Meter {
    Packets(usize),
    Bytes(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PsaObject {
    PacketIn(PacketIn),
    PacketOut(PacketOut),
    Counter(Counter),
    Register(Register),
    Hash(String),
    InternetChecksum(BigInt),
    Meter(Meter),
}

impl PsaObject {
    fn to_external(&self) -> Result<ExternalData, SimError> {
        let (kind, fields) = match self {
            Self::PacketIn(packet) => return Ok(packet.to_external()),
            Self::PacketOut(packet) => return Ok(packet.to_external()),
            Self::Counter(counter) => {
                let (counter_kind, values) = match counter {
                    Counter::Packets(values) => ("packets", encode_bigints(values)),
                    Counter::Bytes(values) => ("bytes", encode_bigints(values)),
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
                    "counter",
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
                "register",
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
            Self::Hash(algorithm) => (
                "hash",
                vec![(
                    "algorithm".to_owned(),
                    ExternalData::String(algorithm.clone()),
                )],
            ),
            Self::InternetChecksum(checksum) => (
                "internet-checksum",
                vec![("value".to_owned(), encode_bigint(checksum))],
            ),
            Self::Meter(meter) => {
                let (meter_kind, size) = match meter {
                    Meter::Packets(size) => ("packets", *size),
                    Meter::Bytes(size) => ("bytes", *size),
                };
                (
                    "meter",
                    vec![
                        (
                            "meter-kind".to_owned(),
                            ExternalData::String(meter_kind.to_owned()),
                        ),
                        ("size".to_owned(), encode_usize(&size)),
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
            return Err(SimError::message("expected PSA object state"));
        };
        let ExternalData::String(kind) = field(fields, "kind")? else {
            return Err(SimError::message("PSA object kind must be a string"));
        };
        match kind.as_str() {
            "packet-in" => PacketIn::from_external(value).map(Self::PacketIn),
            "packet-out" => PacketOut::from_external(value).map(Self::PacketOut),
            "counter" => {
                let ExternalData::String(counter_kind) = field(fields, "counter-kind")? else {
                    return Err(SimError::message("counter kind must be a string"));
                };
                let ExternalData::List(values) = field(fields, "values")? else {
                    return Err(SimError::message("counter values must be a list"));
                };
                let counter = match counter_kind.as_str() {
                    "packets" => Counter::Packets(decode_bigints(values)?),
                    "bytes" => Counter::Bytes(decode_bigints(values)?),
                    "packets-and-bytes" => Counter::PacketsAndBytes(
                        values
                            .iter()
                            .map(|value| {
                                let ExternalData::Tuple(values) = value else {
                                    return Err(SimError::message(
                                        "packet/byte counter value must be a tuple",
                                    ));
                                };
                                let [packets, bytes] = values.as_slice() else {
                                    return Err(SimError::message(
                                        "packet/byte counter value must have two elements",
                                    ));
                                };
                                Ok((decode_bigint(packets)?, decode_bigint(bytes)?))
                            })
                            .collect::<Result<Vec<_>, SimError>>()?,
                    ),
                    _ => return Err(SimError::message("unknown counter kind")),
                };
                Ok(Self::Counter(counter))
            }
            "register" => {
                let typ = decode_value(field(fields, "type")?)?;
                let ExternalData::List(values) = field(fields, "values")? else {
                    return Err(SimError::message("register values must be a list"));
                };
                let values = values
                    .iter()
                    .map(decode_value)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Self::Register(Register { typ, values }))
            }
            "hash" => {
                let ExternalData::String(algorithm) = field(fields, "algorithm")? else {
                    return Err(SimError::message("hash algorithm must be a string"));
                };
                Ok(Self::Hash(algorithm.clone()))
            }
            "internet-checksum" => Ok(Self::InternetChecksum(decode_bigint(field(
                fields, "value",
            )?)?)),
            "meter" => {
                let ExternalData::String(meter_kind) = field(fields, "meter-kind")? else {
                    return Err(SimError::message("meter kind must be a string"));
                };
                let size = decode_usize(field(fields, "size")?)?;
                match meter_kind.as_str() {
                    "packets" => Ok(Self::Meter(Meter::Packets(size))),
                    "bytes" => Ok(Self::Meter(Meter::Bytes(size))),
                    _ => Err(SimError::message("unknown meter kind")),
                }
            }
            _ => Err(SimError::message(format!(
                "unknown PSA object state `{kind}`"
            ))),
        }
    }
}

pub struct Psa;

impl Psa {
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
            "Counter" => {
                let size = fixed_usize(argument("n_counters")?)?;
                let (enum_type, variant) =
                    unpack_p4_enum(argument("type")?).map_err(sim_extern_error)?;
                if enum_type != "PSA_CounterType_t" {
                    return Err(extern_error("invalid PSA counter type"));
                }
                let counter = match variant.as_str() {
                    "PACKETS" => Counter::packets(size),
                    "BYTES" => Counter::bytes(size),
                    "PACKETS_AND_BYTES" => Counter::packets_and_bytes(size),
                    _ => return Err(extern_error("invalid PSA counter type")),
                };
                PsaObject::Counter(counter)
            }
            "Register" => {
                let type_args = get::list(type_args).map_err(value_extern_error)?;
                let [typ, _size_type] = type_args else {
                    return Err(extern_error(
                        "Register constructor expects 2 type arguments",
                    ));
                };
                let size = fixed_usize(argument("size")?)?;
                let initial = if let Ok(value) = argument("initial_value") {
                    value.clone()
                } else {
                    bridge.default_value(typ).map_err(sim_extern_error)?
                };
                PsaObject::Register(Register::new(typ.clone(), size, initial))
            }
            "Hash" => {
                let (enum_type, variant) =
                    unpack_p4_enum(argument("algo")?).map_err(sim_extern_error)?;
                if enum_type != "PSA_HashAlgorithm_t" {
                    return Err(extern_error("invalid PSA hash algorithm"));
                }
                let algorithm = match variant.as_str() {
                    "IDENTITY" => "identity",
                    "CRC32" => "crc32",
                    "CRC16" => "crc16",
                    "ONES_COMPLEMENT16" => "csum16",
                    other => other,
                };
                PsaObject::Hash(algorithm.to_owned())
            }
            "InternetChecksum" => PsaObject::InternetChecksum(BigInt::zero()),
            "Meter" => {
                let size = fixed_usize(argument("n_meters")?)?;
                let (enum_type, variant) =
                    unpack_p4_enum(argument("type")?).map_err(sim_extern_error)?;
                if enum_type != "PSA_MeterType_t" {
                    return Err(extern_error("invalid PSA meter type"));
                }
                let meter = match variant.as_str() {
                    "PACKETS" => Meter::Packets(size),
                    "BYTES" => Meter::Bytes(size),
                    _ => return Err(extern_error("invalid PSA meter type")),
                };
                PsaObject::Meter(meter)
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
        if name != "verify" || parameters.as_slice() != ["check", "toSignal"] {
            return Err(extern_error("unsupported extern function call"));
        }
        let mut bridge = Spec::new(spec);
        let cursor = local_cursor().map_err(sim_extern_error)?;
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
        Ok(vec![context.clone(), architecture.clone(), result])
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
        let object = PsaObject::from_external(get::external(&state).map_err(value_extern_error)?)
            .map_err(sim_extern_error)?;
        let (object, context, architecture, result) = self.object_method(
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
            .update_object_state(&architecture, object_id, &state)
            .map_err(sim_extern_error)?;
        Ok(vec![context, architecture, result])
    }

    fn object_method(
        &mut self,
        spec: &mut Spec<'_>,
        object: PsaObject,
        context: &ValueRef,
        architecture: &ValueRef,
        name: &str,
        parameters: &[String],
    ) -> Result<(PsaObject, ValueRef, ValueRef, ValueRef), ExternError> {
        let cursor = local_cursor().map_err(sim_extern_error)?;
        match object {
            PsaObject::PacketIn(mut packet) => {
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
                Ok((
                    PsaObject::PacketIn(packet),
                    context,
                    architecture.clone(),
                    result,
                ))
            }
            PsaObject::PacketOut(mut packet) => {
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
                    PsaObject::PacketOut(packet),
                    context.clone(),
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                ))
            }
            PsaObject::Counter(mut counter) => {
                if !matches!((name, parameters), ("count", [index]) if index == "index") {
                    return Err(unsupported_method("Counter", name, parameters));
                }
                let index = fixed_usize(
                    &spec
                        .find_var(&cursor, context, "index")
                        .map_err(sim_extern_error)?,
                )?;
                counter.count(index).map_err(sim_extern_error)?;
                Ok((
                    PsaObject::Counter(counter),
                    context.clone(),
                    architecture.clone(),
                    return_result(None).map_err(sim_extern_error)?,
                ))
            }
            PsaObject::Register(mut register) => {
                let index = fixed_usize(
                    &spec
                        .find_var(&cursor, context, "index")
                        .map_err(sim_extern_error)?,
                )?;
                let result = match (name, parameters) {
                    ("read", [index_name]) if index_name == "index" => {
                        let value = match register.read(index) {
                            Some(value) => value.clone(),
                            None => spec
                                .default_value(register.typ())
                                .map_err(sim_extern_error)?,
                        };
                        return_result(Some(value)).map_err(sim_extern_error)?
                    }
                    ("write", [index_name, value_name])
                        if index_name == "index" && value_name == "value" =>
                    {
                        let value = spec
                            .find_var(&cursor, context, "value")
                            .map_err(sim_extern_error)?;
                        register.write(index, value);
                        return_result(None).map_err(sim_extern_error)?
                    }
                    _ => return Err(unsupported_method("Register", name, parameters)),
                };
                Ok((
                    PsaObject::Register(register),
                    context.clone(),
                    architecture.clone(),
                    result,
                ))
            }
            PsaObject::Hash(algorithm) => {
                let data = spec
                    .find_var(&cursor, context, "data")
                    .map_err(sim_extern_error)?;
                let values = unpack_p4_tuple(&data).map_err(sim_extern_error)?;
                let mut result = compute_checksum(&algorithm, &values, &BigInt::zero())
                    .map_err(sim_extern_error)?;
                match (name, parameters) {
                    ("get_hash", [data_name]) if data_name == "data" => {}
                    ("get_hash", [base, data_name, maximum])
                        if base == "base" && data_name == "data" && maximum == "max" =>
                    {
                        let (_, base) = unpack_p4_fixed_bit(
                            &spec
                                .find_var(&cursor, context, "base")
                                .map_err(sim_extern_error)?,
                        )
                        .map_err(sim_extern_error)?;
                        let (_, maximum) = unpack_p4_fixed_bit(
                            &spec
                                .find_var(&cursor, context, "max")
                                .map_err(sim_extern_error)?,
                        )
                        .map_err(sim_extern_error)?;
                        result = base + result % maximum;
                    }
                    _ => return Err(unsupported_method("Hash", name, parameters)),
                }
                let typ = spec
                    .find_type(&cursor, context, "O")
                    .map_err(sim_extern_error)?;
                let value = pack_p4_arbitrary_int(result).map_err(sim_extern_error)?;
                let value = spec.cast(&typ, &value).map_err(sim_extern_error)?;
                Ok((
                    PsaObject::Hash(algorithm),
                    context.clone(),
                    architecture.clone(),
                    return_result(Some(value)).map_err(sim_extern_error)?,
                ))
            }
            PsaObject::InternetChecksum(mut checksum) => {
                let result = match (name, parameters) {
                    ("clear", []) => {
                        checksum = BigInt::zero();
                        return_result(None).map_err(sim_extern_error)?
                    }
                    ("add", [data]) | ("subtract", [data]) if data == "data" => {
                        let data = spec
                            .find_var(&cursor, context, "data")
                            .map_err(sim_extern_error)?;
                        let values = unpack_p4_tuple(&data).map_err(sim_extern_error)?;
                        let algorithm = if name == "add" {
                            "csum16"
                        } else {
                            "csum16_sub"
                        };
                        checksum = compute_checksum(algorithm, &values, &checksum)
                            .map(|value| bitwise_neg(&value, 16))
                            .map_err(sim_extern_error)?;
                        return_result(None).map_err(sim_extern_error)?
                    }
                    ("get", []) => {
                        checksum = bitwise_neg(&checksum, 16);
                        let value = pack_p4_fixed_bit(BigInt::from(16), checksum.clone())
                            .map_err(sim_extern_error)?;
                        return_result(Some(value)).map_err(sim_extern_error)?
                    }
                    ("get_state", []) => {
                        let value = pack_p4_fixed_bit(BigInt::from(16), checksum.clone())
                            .map_err(sim_extern_error)?;
                        return_result(Some(value)).map_err(sim_extern_error)?
                    }
                    ("set_state", [state]) if state == "checksum_state" => {
                        let value = spec
                            .find_var(&cursor, context, "checksum_state")
                            .map_err(sim_extern_error)?;
                        checksum = unpack_p4_fixed_bit(&value)
                            .map(|(_, value)| value)
                            .map_err(sim_extern_error)?;
                        return_result(None).map_err(sim_extern_error)?
                    }
                    _ => {
                        return Err(unsupported_method("InternetChecksum", name, parameters));
                    }
                };
                Ok((
                    PsaObject::InternetChecksum(checksum),
                    context.clone(),
                    architecture.clone(),
                    result,
                ))
            }
            PsaObject::Meter(meter) => {
                if !matches!((name, parameters), ("execute", [index]) if index == "index")
                    && !matches!((name, parameters), ("execute", [index, color]) if index == "index" && color == "color")
                {
                    return Err(unsupported_method("Meter", name, parameters));
                }
                let color = pack_p4_enum("PSA_MeterColor_t", "GREEN").map_err(sim_extern_error)?;
                Ok((
                    PsaObject::Meter(meter),
                    context.clone(),
                    architecture.clone(),
                    return_result(Some(color)).map_err(sim_extern_error)?,
                ))
            }
        }
    }
}

impl Default for Psa {
    fn default() -> Self {
        Self::new()
    }
}

impl Extern for Psa {
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

pub fn transform_stf(statement: StfStmt) -> StfStmt {
    match statement {
        StfStmt::RegisterRead { name, index } => StfStmt::RegisterRead {
            name: transform_register_name(name),
            index,
        },
        StfStmt::RegisterWrite { name, index, value } => StfStmt::RegisterWrite {
            name: transform_register_name(name),
            index,
            value,
        },
        StfStmt::RegisterReset { name } => StfStmt::RegisterReset {
            name: transform_register_name(name),
        },
        statement => statement,
    }
}

fn transform_register_name(name: String) -> String {
    let mut parts = name.split('.');
    let Some(first) = parts.next() else {
        return name;
    };
    if !first.to_ascii_lowercase().contains("ingress") {
        return name;
    }
    std::iter::once("ip.ig")
        .chain(parts)
        .collect::<Vec<_>>()
        .join(".")
}

fn architecture_to_external(state: &ArchitectureState) -> ExternalData {
    let mirrors = state
        .mirrors
        .iter()
        .map(|(session, group)| {
            ExternalData::Tuple(vec![
                ExternalData::Int(i64::from(*session)),
                ExternalData::Int(i64::from(*group)),
            ])
        })
        .collect();
    let groups = state
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
        .collect();
    let nodes = state
        .multicast
        .nodes
        .iter()
        .map(|(handle, nodes)| {
            ExternalData::Tuple(vec![
                ExternalData::Int(i64::from(*handle)),
                ExternalData::List(
                    nodes
                        .iter()
                        .map(|node| {
                            ExternalData::Tuple(vec![
                                ExternalData::Int(i64::from(node.port)),
                                ExternalData::Int(i64::from(node.instance)),
                            ])
                        })
                        .collect(),
                ),
            ])
        })
        .collect();
    ExternalData::Assoc(vec![
        (
            "kind".to_owned(),
            ExternalData::String("psa-architecture".to_owned()),
        ),
        ("mirrors".to_owned(), ExternalData::List(mirrors)),
        (
            "multicast-next-handle".to_owned(),
            ExternalData::Int(i64::from(state.multicast.next_handle)),
        ),
        ("multicast-groups".to_owned(), ExternalData::List(groups)),
        ("multicast-nodes".to_owned(), ExternalData::List(nodes)),
    ])
}

fn architecture_from_external(value: &ExternalData) -> Result<ArchitectureState, SimError> {
    let ExternalData::Assoc(fields) = value else {
        return Err(SimError::message("expected PSA architecture state"));
    };
    if field(fields, "kind")? != &ExternalData::String("psa-architecture".to_owned()) {
        return Err(SimError::message("expected PSA architecture state"));
    }
    let ExternalData::List(mirror_values) = field(fields, "mirrors")? else {
        return Err(SimError::message("PSA mirrors must be a list"));
    };
    let mirrors = decode_pairs(mirror_values)?
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let ExternalData::Int(next_handle) = field(fields, "multicast-next-handle")? else {
        return Err(SimError::message("multicast handle must be an integer"));
    };
    let next_handle = i32::try_from(*next_handle)
        .map_err(|_| SimError::message("multicast handle does not fit i32"))?;
    let ExternalData::List(group_values) = field(fields, "multicast-groups")? else {
        return Err(SimError::message("multicast groups must be a list"));
    };
    let mut groups = BTreeMap::new();
    for value in group_values {
        let ExternalData::Tuple(values) = value else {
            return Err(SimError::message("multicast group must be a tuple"));
        };
        let [group, handles] = values.as_slice() else {
            return Err(SimError::message(
                "multicast group tuple must have two values",
            ));
        };
        let ExternalData::List(handles) = handles else {
            return Err(SimError::message("multicast handles must be a list"));
        };
        groups.insert(decode_i32(group)?, decode_i32s(handles)?);
    }
    let ExternalData::List(node_values) = field(fields, "multicast-nodes")? else {
        return Err(SimError::message("multicast nodes must be a list"));
    };
    let mut nodes = BTreeMap::new();
    for value in node_values {
        let ExternalData::Tuple(values) = value else {
            return Err(SimError::message("multicast node must be a tuple"));
        };
        let [handle, replicas] = values.as_slice() else {
            return Err(SimError::message(
                "multicast node tuple must have two values",
            ));
        };
        let ExternalData::List(replicas) = replicas else {
            return Err(SimError::message("multicast replicas must be a list"));
        };
        let replicas = decode_pairs(replicas)?
            .into_iter()
            .map(|(port, instance)| MulticastNode { port, instance })
            .collect();
        nodes.insert(decode_i32(handle)?, replicas);
    }
    Ok(ArchitectureState {
        mirrors,
        multicast: MulticastState {
            next_handle,
            groups,
            nodes,
        },
    })
}

fn encode_value(value: &ValueRef) -> Result<ExternalData, SimError> {
    let canonical = runtime_value::to_canonical(value);
    let json =
        ValueCodec::encode(&canonical).map_err(|error| SimError::message(error.to_string()))?;
    serde_json::to_string(&json)
        .map(ExternalData::String)
        .map_err(|error| SimError::message(error.to_string()))
}

fn decode_value(value: &ExternalData) -> Result<ValueRef, SimError> {
    let ExternalData::String(json) = value else {
        return Err(SimError::message("encoded runtime value must be a string"));
    };
    let json = serde_json::from_str(json).map_err(|error| SimError::message(error.to_string()))?;
    let value = ValueCodec::decode(&json).map_err(|error| SimError::message(error.to_string()))?;
    Ok(runtime_value::to_runtime(&value))
}

fn fixed_usize(value: &ValueRef) -> Result<usize, ExternError> {
    unpack_p4_fixed_bit(value)
        .map_err(sim_extern_error)?
        .1
        .to_usize()
        .ok_or_else(|| extern_error("fixed-width number does not fit usize"))
}

fn encode_bigint(value: &BigInt) -> ExternalData {
    ExternalData::Intlit(value.to_string())
}

fn encode_bigints(values: &[BigInt]) -> ExternalData {
    ExternalData::List(values.iter().map(encode_bigint).collect())
}

fn decode_bigint(value: &ExternalData) -> Result<BigInt, SimError> {
    match value {
        ExternalData::Int(value) => Ok(BigInt::from(*value)),
        ExternalData::Intlit(value) => BigInt::parse_bytes(value.as_bytes(), 10)
            .ok_or_else(|| SimError::message("invalid bigint object state")),
        _ => Err(SimError::message("object-state bigint has invalid type")),
    }
}

fn decode_bigints(values: &[ExternalData]) -> Result<Vec<BigInt>, SimError> {
    values.iter().map(decode_bigint).collect()
}

fn encode_usize(value: &usize) -> ExternalData {
    u64::try_from(*value)
        .ok()
        .and_then(|value| i64::try_from(value).ok())
        .map(ExternalData::Int)
        .unwrap_or_else(|| ExternalData::Intlit(value.to_string()))
}

fn decode_usize(value: &ExternalData) -> Result<usize, SimError> {
    decode_bigint(value)?
        .to_usize()
        .ok_or_else(|| SimError::message("object-state size does not fit usize"))
}

fn decode_i32(value: &ExternalData) -> Result<i32, SimError> {
    let ExternalData::Int(value) = value else {
        return Err(SimError::message("architecture integer has invalid type"));
    };
    i32::try_from(*value).map_err(|_| SimError::message("architecture integer does not fit i32"))
}

fn decode_i32s(values: &[ExternalData]) -> Result<Vec<i32>, SimError> {
    values.iter().map(decode_i32).collect()
}

fn decode_pairs(values: &[ExternalData]) -> Result<Vec<(i32, i32)>, SimError> {
    values
        .iter()
        .map(|value| {
            let ExternalData::Tuple(values) = value else {
                return Err(SimError::message("architecture pair must be a tuple"));
            };
            let [first, second] = values.as_slice() else {
                return Err(SimError::message("architecture pair must have two values"));
            };
            Ok((decode_i32(first)?, decode_i32(second)?))
        })
        .collect()
}

fn field<'a>(
    fields: &'a [(String, ExternalData)],
    name: &str,
) -> Result<&'a ExternalData, SimError> {
    fields
        .iter()
        .find_map(|(field, value)| (field == name).then_some(value))
        .ok_or_else(|| SimError::message(format!("missing PSA state field `{name}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::value::make;

    #[test]
    fn register_object_state_round_trips_runtime_values() {
        let typ = make::text("T".to_owned(), crate::domain::source::Region::none());
        let initial = make::int(BigInt::from(7), crate::domain::source::Region::none());
        let object = PsaObject::Register(Register::new(typ, 2, initial));

        let external = object.to_external().unwrap();

        assert_eq!(PsaObject::from_external(&external).unwrap(), object);
    }

    #[test]
    fn architecture_state_round_trips_mirror_and_multicast_tables() {
        let mut state = ArchitectureState::default();
        state.mirrors.insert(3, 100);
        state.multicast.create_group(100);
        let handle = state.multicast.create_node(8, vec![2, 4]);
        state.multicast.associate(100, handle);

        let external = architecture_to_external(&state);

        assert_eq!(architecture_from_external(&external).unwrap(), state);
    }
}
