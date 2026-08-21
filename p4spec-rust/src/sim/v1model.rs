use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};

use crate::{
    domain::external_data::ExternalData,
    interface::{Extern, ExternError, SpecCall},
    lang::il::ast::Typ,
    runtime::value::{ValueRef, get},
};

use super::{
    SimError,
    core::{
        PacketIn, PacketOut, extern_error, external_value, pack_p4_fixed_bit, packet_advance,
        packet_extract, packet_lookahead, reject_result, return_result, sim_extern_error,
        text_list, unpack_p4_bool, unpack_p4_enum, unpack_p4_fixed_bit, unsupported_method,
        value_extern_error,
    },
    psa::{Counter, Register, decode_bigint, decode_value, encode_bigint, encode_value},
    spec::{Spec, local_cursor, prefixed_name},
};

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
        if name != "verify" || parameters.as_slice() != ["check", "toSignal"] {
            return Err(extern_error(format!(
                "unsupported extern function call: {name}({})",
                parameters.join(", ")
            )));
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
            "init_archState" => Ok(external_value("archState", ExternalData::Null)),
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
}
