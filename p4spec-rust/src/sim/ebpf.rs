use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::{
    domain::{
        atom::Atom,
        external_data::ExternalData,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interface::{Extern, ExternError, SpecCall},
    lang::il::ast::Typ,
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
    wire::sim_suite::{StfAction, StfStmt},
};

use super::{
    SimError,
    architecture::Architecture,
    core::{PacketIn, pack_p4_fixed_bit, unpack_p4_bool, unpack_p4_fixed_bit},
    io::{Rx, Tx},
    spec::{Spec, global_cursor, local_cursor, prefixed_name},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CounterArray {
    values: Vec<i64>,
}

impl CounterArray {
    pub fn new(max_index: usize, _sparse: bool) -> Self {
        Self {
            values: vec![0; max_index],
        }
    }

    pub fn values(&self) -> &[i64] {
        &self.values
    }

    pub fn increment(&mut self, index: usize) {
        self.add(index, 1);
    }

    pub fn add(&mut self, index: usize, value: i64) {
        if let Some(counter) = self.values.get_mut(index) {
            *counter += value;
        }
    }

    fn to_external(&self) -> ExternalData {
        ExternalData::Assoc(vec![
            (
                "kind".to_owned(),
                ExternalData::String("counter-array".to_owned()),
            ),
            (
                "values".to_owned(),
                ExternalData::List(self.values.iter().copied().map(ExternalData::Int).collect()),
            ),
        ])
    }

    fn from_external(value: &ExternalData) -> Result<Self, SimError> {
        let ExternalData::Assoc(fields) = value else {
            return Err(SimError::message("expected counter-array object state"));
        };
        let kind = field(fields, "kind")?;
        if kind != &ExternalData::String("counter-array".to_owned()) {
            return Err(SimError::message("expected counter-array object state"));
        }
        let ExternalData::List(values) = field(fields, "values")? else {
            return Err(SimError::message("counter-array values must be a list"));
        };
        let values = values
            .iter()
            .map(|value| match value {
                ExternalData::Int(value) => Ok(*value),
                _ => Err(SimError::message("counter-array value must be an integer")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { values })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EbpfObject {
    PacketIn(PacketIn),
    CounterArray(CounterArray),
}

impl EbpfObject {
    fn to_external(&self) -> ExternalData {
        match self {
            Self::PacketIn(packet) => packet.to_external(),
            Self::CounterArray(counters) => counters.to_external(),
        }
    }

    fn from_external(value: &ExternalData) -> Result<Self, SimError> {
        let ExternalData::Assoc(fields) = value else {
            return Err(SimError::message("expected eBPF object state"));
        };
        match field(fields, "kind")? {
            ExternalData::String(kind) if kind == "packet-in" => {
                PacketIn::from_external(value).map(Self::PacketIn)
            }
            ExternalData::String(kind) if kind == "counter-array" => {
                CounterArray::from_external(value).map(Self::CounterArray)
            }
            _ => Err(SimError::message("unknown eBPF object state")),
        }
    }
}

pub struct Ebpf;

impl Ebpf {
    pub fn new() -> Self {
        Self
    }

    fn eval_extern_init(&mut self, values: &[ValueRef]) -> Result<ValueRef, ExternError> {
        let [name, _type_args, ids, arguments] = values else {
            return Err(error("unexpected number of arguments to extern init"));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        if name != "CounterArray" {
            return Ok(external_value("objectState", ExternalData::Null));
        }
        let ids = text_list(ids)?;
        let arguments = get::list(arguments).map_err(value_extern_error)?;
        if ids.len() != arguments.len() {
            return Err(error("counter-array argument name/value count mismatch"));
        }
        let find = |name: &str| {
            ids.iter()
                .position(|id| id == name)
                .and_then(|index| arguments.get(index))
                .ok_or_else(|| error(format!("missing CounterArray argument `{name}`")))
        };
        let (_, max_index) = unpack_p4_fixed_bit(find("max_index")?).map_err(sim_extern_error)?;
        let sparse = unpack_p4_bool(find("sparse")?).map_err(sim_extern_error)?;
        let max_index = max_index
            .to_usize()
            .ok_or_else(|| error("CounterArray max_index does not fit usize"))?;
        let counters = CounterArray::new(max_index, sparse);
        Ok(external_value(
            "objectState",
            EbpfObject::CounterArray(counters).to_external(),
        ))
    }

    fn eval_compile_time_function(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        let [context, name, parameters] = values else {
            return Err(error(
                "unexpected number of arguments to local compile-time known extern function call",
            ));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let parameters = text_list(parameters)?;
        let has_message = match (name, parameters.as_slice()) {
            ("static_assert", [check, message]) if check == "check" && message == "message" => true,
            ("static_assert", [check]) if check == "check" => false,
            _ => {
                return Err(error(format!(
                    "unsupported local compile-time known extern function call: {name}({})",
                    parameters.join(", ")
                )));
            }
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
        Err(error(message))
    }

    fn eval_extern_function(
        &mut self,
        spec: &mut dyn SpecCall,
        values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        let [context, architecture, name, parameters] = values else {
            return Err(error(
                "unexpected number of arguments to extern function call",
            ));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let parameters = text_list(parameters)?;
        if name != "verify" || parameters.as_slice() != ["check", "toSignal"] {
            return Err(error(format!(
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
            return Err(error(
                "unexpected number of arguments to extern method call",
            ));
        };
        let name = get::text(name).map_err(value_extern_error)?;
        let parameters = text_list(parameters)?;
        let mut bridge = Spec::new(spec);
        let state = bridge
            .find_object_state(architecture, object_id)
            .map_err(sim_extern_error)?;
        let external = get::external(&state).map_err(value_extern_error)?;
        let object = EbpfObject::from_external(external).map_err(sim_extern_error)?;
        let (object, context, architecture, result) = match object {
            EbpfObject::PacketIn(packet) => self.packet_method(
                &mut bridge,
                packet,
                context,
                architecture,
                name,
                &parameters,
            )?,
            EbpfObject::CounterArray(counters) => self.counter_method(
                &mut bridge,
                counters,
                context,
                architecture,
                name,
                &parameters,
            )?,
        };
        let state = external_value("objectState", object.to_external());
        let architecture = bridge
            .update_object_state(&architecture, object_id, &state)
            .map_err(sim_extern_error)?;
        Ok(vec![context, architecture, result])
    }

    fn packet_method(
        &mut self,
        spec: &mut Spec<'_>,
        mut packet: PacketIn,
        context: &ValueRef,
        architecture: &ValueRef,
        name: &str,
        parameters: &[String],
    ) -> Result<(EbpfObject, ValueRef, ValueRef, ValueRef), ExternError> {
        let (context, result) = match (name, parameters) {
            ("extract", [header]) if header == "hdr" => {
                packet_extract(spec, &mut packet, context, architecture, false)?
            }
            ("extract", [header, size])
                if header == "variableSizeHeader" && size == "variableFieldSizeInBits" =>
            {
                packet_extract(spec, &mut packet, context, architecture, true)?
            }
            ("lookahead", []) => packet_lookahead(spec, &packet, context, architecture)?,
            ("advance", [size]) if size == "sizeInBits" => {
                packet_advance(spec, &mut packet, context)?
            }
            ("length", []) => {
                let length = pack_p4_fixed_bit(BigInt::from(32), BigInt::from(packet.len_bytes()))
                    .map_err(sim_extern_error)?;
                (
                    context.clone(),
                    return_result(Some(length)).map_err(sim_extern_error)?,
                )
            }
            _ => return Err(unsupported_method("packet_in", name, parameters)),
        };
        Ok((
            EbpfObject::PacketIn(packet),
            context,
            architecture.clone(),
            result,
        ))
    }

    fn counter_method(
        &mut self,
        spec: &mut Spec<'_>,
        mut counters: CounterArray,
        context: &ValueRef,
        architecture: &ValueRef,
        name: &str,
        parameters: &[String],
    ) -> Result<(EbpfObject, ValueRef, ValueRef, ValueRef), ExternError> {
        let cursor = local_cursor().map_err(sim_extern_error)?;
        let index = spec
            .find_var(&cursor, context, "index")
            .map_err(sim_extern_error)?;
        let (_, index) = unpack_p4_fixed_bit(&index).map_err(sim_extern_error)?;
        let index = index
            .to_usize()
            .ok_or_else(|| error("counter index does not fit usize"))?;
        match (name, parameters) {
            ("increment", [index_name]) if index_name == "index" => counters.increment(index),
            ("add", [index_name, value_name]) if index_name == "index" && value_name == "value" => {
                let value = spec
                    .find_var(&cursor, context, "value")
                    .map_err(sim_extern_error)?;
                let (_, value) = unpack_p4_fixed_bit(&value).map_err(sim_extern_error)?;
                counters.add(
                    index,
                    value
                        .to_i64()
                        .ok_or_else(|| error("counter value does not fit i64"))?,
                );
            }
            _ => return Err(unsupported_method("CounterArray", name, parameters)),
        }
        Ok((
            EbpfObject::CounterArray(counters),
            context.clone(),
            architecture.clone(),
            return_result(None).map_err(sim_extern_error)?,
        ))
    }
}

impl Default for Ebpf {
    fn default() -> Self {
        Self::new()
    }
}

impl Extern for Ebpf {
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
            _ => Err(error(format!("unimplemented extern relation: {name}"))),
        }
    }

    fn eval_func(
        &mut self,
        _spec: &mut dyn SpecCall,
        name: &str,
        _type_args: &[Typ],
        values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        match name {
            "init_objectState" => self.eval_extern_init(values),
            "init_archState" => Ok(external_value("archState", ExternalData::Null)),
            _ => Err(error(format!("unimplemented extern function: {name}"))),
        }
    }

    fn checkpoint(&self) -> u64 {
        0
    }

    fn clear(&mut self) {}
}

impl Architecture for Ebpf {
    fn name() -> &'static str {
        "ebpf"
    }

    fn init(spec: &mut dyn SpecCall, program: &ValueRef) -> Result<(ValueRef, ValueRef), SimError> {
        Spec::new(spec).ebpf_init(program)
    }

    fn drive(
        spec: &mut dyn SpecCall,
        context: ValueRef,
        architecture: ValueRef,
        rx: Rx,
    ) -> Result<(ValueRef, ValueRef, Vec<Tx>), SimError> {
        let packet = PacketIn::new(&rx.packet)?;
        let packet = external_value("objectState", EbpfObject::PacketIn(packet).to_external());
        let mut bridge = Spec::new(spec);
        let (context, architecture) =
            bridge.ebpf_init_packet_in(&context, &architecture, &packet)?;
        let context = bridge.ebpf_init_globals(&context, &architecture)?;
        let (context, architecture, parse_result) = bridge.ebpf_parse(&context, &architecture)?;
        if is_reject(&parse_result)? {
            return Ok((context, architecture, Vec::new()));
        }
        let (context, architecture, _) = bridge.ebpf_filter(&context, &architecture)?;
        let cursor = global_cursor()?;
        let accept_reference = prefixed_name("accept")?;
        let accept = bridge.lvalue_read(&cursor, &context, &architecture, &accept_reference)?;
        let outputs = if unpack_p4_bool(&accept)? {
            vec![Tx::new(rx.port, rx.packet)]
        } else {
            Vec::new()
        };
        Ok((context, architecture, outputs))
    }

    fn transform_stf(statement: StfStmt) -> StfStmt {
        transform_stf(statement)
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
            matches,
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
    let name = replace_first_caseless(name, "pipe_c1_", "main.filt.c1.");
    let name = replace_first_caseless(name, "pipe_", "main.filt.");
    replace_first_caseless(name, "pipe", "main.filt")
}

fn transform_action(mut action: StfAction) -> StfAction {
    let name = replace_first_caseless(action.name, "pipe_c1_", "main.filt.c1.");
    let name = replace_first_caseless(name, "pipe_", "main.filt.");
    let name = replace_first_caseless(name, "_NoAction", "NoAction");
    action.name = name.rsplit('.').next().unwrap_or(&name).to_owned();
    action
}

fn replace_first_caseless(mut value: String, pattern: &str, replacement: &str) -> String {
    let lowercase = value.to_ascii_lowercase();
    let Some(index) = lowercase.find(&pattern.to_ascii_lowercase()) else {
        return value;
    };
    value.replace_range(index..index + pattern.len(), replacement);
    value
}

fn field<'a>(
    fields: &'a [(String, ExternalData)],
    name: &str,
) -> Result<&'a ExternalData, SimError> {
    fields
        .iter()
        .find_map(|(field, value)| (field == name).then_some(value))
        .ok_or_else(|| SimError::message(format!("missing counter-array field `{name}`")))
}

fn packet_extract(
    spec: &mut Spec<'_>,
    packet: &mut PacketIn,
    context: &ValueRef,
    architecture: &ValueRef,
    variable: bool,
) -> Result<(ValueRef, ValueRef), ExternError> {
    let cursor = local_cursor().map_err(sim_extern_error)?;
    let typ = required_optional(
        spec.find_type(&cursor, context, "T")
            .map_err(sim_extern_error)?,
        "find_type_e",
    )?;
    let typ = spec
        .substitute_type(&cursor, context, &typ)
        .map_err(sim_extern_error)?;
    let (target_name, variable_size, size) = if variable {
        let minimum = spec
            .sizeof_min_bits(&typ)
            .map_err(sim_extern_error)?
            .to_usize()
            .ok_or_else(|| error("minimum header size does not fit usize"))?;
        let maximum = spec
            .sizeof_max_bits(&typ)
            .map_err(sim_extern_error)?
            .to_usize()
            .ok_or_else(|| error("maximum header size does not fit usize"))?;
        let size_value = spec
            .find_var(&cursor, context, "variableFieldSizeInBits")
            .map_err(sim_extern_error)?;
        let high = super::core::pack_p4_arbitrary_int(BigInt::from(2)).map_err(sim_extern_error)?;
        let low = super::core::pack_p4_arbitrary_int(BigInt::from(0)).map_err(sim_extern_error)?;
        let alignment = spec
            .bitacc_range(&size_value, &high, &low)
            .and_then(|value| unpack_p4_fixed_bit(&value).map(|(_, value)| value))
            .map_err(sim_extern_error)?;
        let (_, variable_size) = unpack_p4_fixed_bit(&size_value).map_err(sim_extern_error)?;
        let variable_size = variable_size
            .to_usize()
            .ok_or_else(|| error("variable header size does not fit usize"))?;
        let size = minimum
            .checked_add(variable_size)
            .ok_or_else(|| error("variable header size overflow"))?;
        if alignment != BigInt::from(0) {
            return Ok((context.clone(), reject_error("ParserInvalidArgument")?));
        }
        if size > maximum {
            return Ok((context.clone(), reject_error("HeaderTooShort")?));
        }
        ("variableSizeHeader", variable_size, size)
    } else {
        let size = spec
            .sizeof_max_bits(&typ)
            .map_err(sim_extern_error)?
            .to_usize()
            .ok_or_else(|| error("header size does not fit usize"))?;
        ("hdr", 0, size)
    };
    let bits = match packet.take(size) {
        Ok(bits) => bits,
        Err(_) => return Ok((context.clone(), reject_error("PacketTooShort")?)),
    };
    let target = spec
        .find_var(&cursor, context, target_name)
        .map_err(sim_extern_error)?;
    let target = spec
        .write_value_from_bits(&target, variable_size, &bits)
        .map_err(sim_extern_error)?;
    let reference = prefixed_name(target_name).map_err(sim_extern_error)?;
    let context = spec
        .lvalue_write(&cursor, context, architecture, &reference, &target)
        .map_err(sim_extern_error)?;
    Ok((context, return_result(None).map_err(sim_extern_error)?))
}

fn packet_lookahead(
    spec: &mut Spec<'_>,
    packet: &PacketIn,
    context: &ValueRef,
    _architecture: &ValueRef,
) -> Result<(ValueRef, ValueRef), ExternError> {
    let cursor = local_cursor().map_err(sim_extern_error)?;
    let typ = required_optional(
        spec.find_type(&cursor, context, "T")
            .map_err(sim_extern_error)?,
        "find_type_e",
    )?;
    let substituted = spec
        .substitute_type(&cursor, context, &typ)
        .map_err(sim_extern_error)?;
    let size = spec
        .sizeof_max_bits(&substituted)
        .map_err(sim_extern_error)?
        .to_usize()
        .ok_or_else(|| error("lookahead size does not fit usize"))?;
    let mut preview = packet.clone();
    let bits = match preview.take(size) {
        Ok(bits) => bits,
        Err(_) => return Ok((context.clone(), reject_error("PacketTooShort")?)),
    };
    let value = spec.default_value(&typ).map_err(sim_extern_error)?;
    let value = spec
        .write_value_from_bits(&value, 0, &bits)
        .map_err(sim_extern_error)?;
    Ok((
        context.clone(),
        return_result(Some(value)).map_err(sim_extern_error)?,
    ))
}

fn packet_advance(
    spec: &mut Spec<'_>,
    packet: &mut PacketIn,
    context: &ValueRef,
) -> Result<(ValueRef, ValueRef), ExternError> {
    let cursor = local_cursor().map_err(sim_extern_error)?;
    let size = spec
        .find_var(&cursor, context, "sizeInBits")
        .map_err(sim_extern_error)?;
    let (_, size) = unpack_p4_fixed_bit(&size).map_err(sim_extern_error)?;
    let size = size
        .to_usize()
        .ok_or_else(|| error("packet advance does not fit usize"))?;
    if packet.advance(size).is_err() {
        return Ok((context.clone(), reject_error("PacketTooShort")?));
    }
    Ok((
        context.clone(),
        return_result(None).map_err(sim_extern_error)?,
    ))
}

fn required_optional(value: ValueRef, name: &str) -> Result<ValueRef, ExternError> {
    get::opt(&value)
        .map_err(value_extern_error)?
        .cloned()
        .ok_or_else(|| error(format!("{name} returned none")))
}

fn return_result(value: Option<ValueRef>) -> Result<ValueRef, SimError> {
    let value_type = named_type("value");
    let optional_type = make_type::opt_type(value_type);
    let value = make::opt(&optional_type, value, Region::none());
    case_value(
        "returnResult",
        Mixfix::Seq(vec![keyword("RETURN"), Mixfix::Arg(())]),
        [value],
    )
}

fn reject_error(name: &str) -> Result<ValueRef, ExternError> {
    let error_value = case_value(
        "errorValue",
        Mixfix::Seq(vec![
            keyword("ERROR"),
            Mixfix::Atom(Spanned::new(Atom::Operator(".".to_owned()), Region::none())),
            Mixfix::Arg(()),
        ]),
        [make::text(name.to_owned(), Region::none())],
    )
    .map_err(sim_extern_error)?;
    reject_result(error_value, "rejectTransitionResult").map_err(sim_extern_error)
}

fn reject_result(value: ValueRef, type_name: &str) -> Result<ValueRef, SimError> {
    case_value(
        type_name,
        Mixfix::Seq(vec![keyword("REJECT"), Mixfix::Arg(())]),
        [value],
    )
}

fn is_reject(value: &ValueRef) -> Result<bool, SimError> {
    let value_case = get::case(value).map_err(value_error)?;
    Ok(value_case.split().0 == Mixfix::Seq(vec![keyword("REJECT"), Mixfix::Arg(())]))
}

fn external_value(type_name: &str, value: ExternalData) -> ValueRef {
    make::external(&named_type(type_name), value, Region::none())
}

fn case_value(
    type_name: &str,
    mixop: Mixop,
    arguments: impl IntoIterator<Item = ValueRef>,
) -> Result<ValueRef, SimError> {
    let value_case =
        Mixop::fill(&mixop, arguments).map_err(|error| SimError::message(error.to_string()))?;
    Ok(make::case(
        &named_type(type_name),
        value_case,
        Region::none(),
    ))
}

fn named_type(name: &str) -> Typ {
    make_type::var_type(Spanned::new(name.to_owned(), Region::none()), Vec::new())
}

fn keyword(value: &str) -> Mixop {
    Mixfix::Atom(Spanned::new(
        Atom::Keyword(value.to_owned()),
        Region::none(),
    ))
}

fn text_list(value: &ValueRef) -> Result<Vec<String>, ExternError> {
    get::list(value)
        .map_err(value_extern_error)?
        .iter()
        .map(|value| {
            get::text(value)
                .map(str::to_owned)
                .map_err(value_extern_error)
        })
        .collect()
}

fn unsupported_method(object: &str, name: &str, parameters: &[String]) -> ExternError {
    error(format!(
        "unsupported extern method call: {object}.{name}({})",
        parameters.join(", ")
    ))
}

fn error(message: impl Into<String>) -> ExternError {
    ExternError::new(Region::none(), message)
}

fn sim_extern_error(error: SimError) -> ExternError {
    ExternError::new(Region::none(), error.to_string())
}

fn value_extern_error(error: impl ToString) -> ExternError {
    ExternError::new(Region::none(), error.to_string())
}

fn value_error(error: impl ToString) -> SimError {
    SimError::message(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_error_uses_the_spec_error_value_mixop() {
        let reject = reject_error("PacketTooShort").unwrap();
        let reject_case = get::case(&reject).unwrap();
        let arguments = reject_case.args();
        let [error] = arguments.as_slice() else {
            panic!("reject result must contain one error value");
        };
        let error_case = get::case(error).unwrap();

        assert_eq!(
            error_case.split().0,
            Mixfix::Seq(vec![
                keyword("ERROR"),
                Mixfix::Atom(Spanned::new(Atom::Operator(".".to_owned()), Region::none(),)),
                Mixfix::Arg(()),
            ])
        );
    }
}
