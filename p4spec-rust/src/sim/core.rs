use num_bigint::BigInt;
use num_traits::{One, Zero};

use crate::domain::external_data::ExternalData;
use crate::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    lang::{il::ast::Typ, xl::num},
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
    wire::sim_suite::{StfAction, StfMatch, StfMatchValue},
};

use super::{SimError, spec::Spec};

pub type Bits = Vec<bool>;

pub fn normalize_key_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut characters = name.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '$'
            || !characters
                .peek()
                .is_some_and(|character| character.is_ascii_digit())
        {
            normalized.push(character);
            continue;
        }
        normalized.push('[');
        while characters
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            normalized.push(characters.next().expect("peeked character"));
        }
        normalized.push(']');
    }
    normalized
}

pub fn add_table_entry(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    table_name: &str,
    priority: Option<i32>,
    matches: &[StfMatch],
    action: &StfAction,
) -> Result<ValueRef, SimError> {
    let table = find_table(spec, architecture, table_name)?;
    let priority = encode_priority(priority);
    let keys = encode_keys(matches)?;
    let action = encode_action(action)?;
    let updated = match spec.table_add_entry(context, &table, &priority, &keys, &action)? {
        Some(updated) => updated,
        None => {
            let names = spec
                .key_interface(&table)?
                .into_iter()
                .filter_map(|(name, match_kind, _typ)| {
                    (get::text(&match_kind).ok() != Some("selector")).then_some(name)
                })
                .collect::<Vec<_>>();
            let values = get::list(&keys)
                .map_err(value_error)?
                .iter()
                .map(|key| {
                    get::two(get::tuple(key).map_err(value_error)?)
                        .map(|(_, value)| value.clone())
                        .map_err(value_error)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if names.len() != values.len() {
                return Err(SimError::message(format!(
                    "table key count mismatch: expected {}, got {}",
                    names.len(),
                    values.len()
                )));
            }
            let key_type = named_type("tableKeyInterface");
            let key_list_type = make_type::list_type(key_type.clone());
            let keys = names
                .into_iter()
                .zip(values)
                .map(|(name, value)| make::tuple(&key_type, vec![name, value], Region::none()))
                .collect();
            let keys = make::list(&key_list_type, keys, Region::none());
            spec.table_add_entry(context, &table, &priority, &keys, &action)?
                .ok_or_else(|| SimError::message("table entry did not match table keys"))?
        }
    };
    update_table(spec, architecture, table_name, &updated)
}

pub fn set_table_default(
    spec: &mut Spec<'_>,
    context: &ValueRef,
    architecture: &ValueRef,
    table_name: &str,
    action: &StfAction,
) -> Result<ValueRef, SimError> {
    let table = find_table(spec, architecture, table_name)?;
    let action = encode_action(action)?;
    let updated = spec.table_add_default_action(context, &table, &action)?;
    update_table(spec, architecture, table_name, &updated)
}

fn find_table(
    spec: &mut Spec<'_>,
    architecture: &ValueRef,
    table_name: &str,
) -> Result<ValueRef, SimError> {
    let names = table_name.split('.').collect::<Vec<_>>();
    let unqualified = names
        .last()
        .ok_or_else(|| SimError::message("empty table name"))?;
    if names.len() > 1 {
        let object_id = encode_object_id(&names);
        if let Some(table) = spec.find_object_qualified(architecture, &object_id)? {
            return Ok(table);
        }
    }
    let unqualified = make::text((*unqualified).to_owned(), Region::none());
    spec.find_object_unqualified(architecture, &unqualified)?
        .ok_or_else(|| SimError::message(format!("table `{table_name}` was not found")))
}

fn update_table(
    spec: &mut Spec<'_>,
    architecture: &ValueRef,
    table_name: &str,
    table: &ValueRef,
) -> Result<ValueRef, SimError> {
    let names = table_name.split('.').collect::<Vec<_>>();
    let unqualified = names
        .last()
        .ok_or_else(|| SimError::message("empty table name"))?;
    if names.len() > 1 {
        let object_id = encode_object_id(&names);
        if spec
            .find_object_qualified(architecture, &object_id)?
            .is_some()
        {
            return spec.update_object_qualified(architecture, &object_id, table);
        }
    }
    let unqualified = make::text((*unqualified).to_owned(), Region::none());
    spec.update_object_unqualified(architecture, &unqualified, table)
}

fn encode_object_id(names: &[&str]) -> ValueRef {
    let name_type = named_type("nameIR");
    let object_id_type = make_type::list_type(name_type);
    let names = names
        .iter()
        .map(|name| make::text((*name).to_owned(), Region::none()))
        .collect();
    make::list(&object_id_type, names, Region::none())
}

fn encode_priority(priority: Option<i32>) -> ValueRef {
    let priority_type = make_type::opt_type(make_type::int_type());
    let priority = priority.map(|priority| make::int(BigInt::from(priority), Region::none()));
    make::opt(&priority_type, priority, Region::none())
}

fn encode_keys(matches: &[StfMatch]) -> Result<ValueRef, SimError> {
    let key_type = named_type("tableKeyInterface");
    let key_list_type = make_type::list_type(key_type.clone());
    let keys = matches
        .iter()
        .map(|entry| {
            let name = make::text(normalize_key_name(&entry.name), Region::none());
            let value = match &entry.value {
                StfMatchValue::Num { value } if value.starts_with("0x") => case_value(
                    "tableKeyValueInterface",
                    Mixfix::Seq(vec![tag("HEX"), Mixfix::Arg(())]),
                    [make::text(value[2..].to_owned(), Region::none())],
                )?,
                StfMatchValue::Num { value } if value.starts_with("0b") => case_value(
                    "tableKeyValueInterface",
                    Mixfix::Seq(vec![tag("BIN"), Mixfix::Arg(())]),
                    [make::text(value[2..].to_owned(), Region::none())],
                )?,
                StfMatchValue::Num { value } => case_value(
                    "tableKeyValueInterface",
                    Mixfix::Seq(vec![tag("DEC"), Mixfix::Arg(())]),
                    [make::text(value.clone(), Region::none())],
                )?,
                StfMatchValue::Slash { prefix, mask } => case_value(
                    "tableKeyValueInterface",
                    Mixfix::Seq(vec![Mixfix::Arg(()), tag("SLASH"), Mixfix::Arg(())]),
                    [
                        make::text(prefix.clone(), Region::none()),
                        make::nat(parse_bigint(mask)?, Region::none()),
                    ],
                )?,
            };
            Ok(make::tuple(&key_type, vec![name, value], Region::none()))
        })
        .collect::<Result<Vec<_>, SimError>>()?;
    Ok(make::list(&key_list_type, keys, Region::none()))
}

fn encode_action(action: &StfAction) -> Result<ValueRef, SimError> {
    let argument_type = named_type("tableActionArgumentInterface");
    let argument_list_type = make_type::list_type(argument_type.clone());
    let arguments = action
        .args
        .iter()
        .map(|argument| {
            Ok(make::tuple(
                &argument_type,
                vec![
                    make::text(argument.id.clone(), Region::none()),
                    make::int(parse_bigint(&argument.number)?, Region::none()),
                ],
                Region::none(),
            ))
        })
        .collect::<Result<Vec<_>, SimError>>()?;
    let arguments = make::list(&argument_list_type, arguments, Region::none());
    Ok(make::tuple(
        &named_type("tableActionInterface"),
        vec![make::text(action.name.clone(), Region::none()), arguments],
        Region::none(),
    ))
}

fn parse_bigint(value: &str) -> Result<BigInt, SimError> {
    let (radix, digits) = if let Some(value) = value.strip_prefix("0x") {
        (16, value)
    } else if let Some(value) = value.strip_prefix("0b") {
        (2, value)
    } else {
        (10, value)
    };
    BigInt::parse_bytes(digits.as_bytes(), radix)
        .ok_or_else(|| SimError::message(format!("invalid integer `{value}`")))
}

pub fn pack_p4_bool(value: bool) -> Result<ValueRef, SimError> {
    case_value(
        "value",
        Mixfix::Seq(vec![tag("B"), Mixfix::Arg(())]),
        [make::bool(value, Region::none())],
    )
}

pub fn unpack_p4_bool(value: &ValueRef) -> Result<bool, SimError> {
    let arguments = case_arguments(value, Mixfix::Seq(vec![tag("B"), Mixfix::Arg(())]))?;
    let [value] = arguments.as_slice() else {
        return Err(SimError::message("expected a P4 bool"));
    };
    get::bool(value).map_err(value_error)
}

pub fn pack_p4_arbitrary_int(value: BigInt) -> Result<ValueRef, SimError> {
    case_value(
        "value",
        Mixfix::Seq(vec![keyword("D"), Mixfix::Arg(())]),
        [make::int(value, Region::none())],
    )
}

pub fn pack_p4_fixed_bit(width: BigInt, value: BigInt) -> Result<ValueRef, SimError> {
    case_value(
        "value",
        fixed_number_mixop("W"),
        [
            make::nat(width, Region::none()),
            make::int(value, Region::none()),
        ],
    )
}

pub fn unpack_p4_fixed_bit(value: &ValueRef) -> Result<(BigInt, BigInt), SimError> {
    unpack_p4_fixed_number(value, "W")
}

pub fn pack_p4_fixed_int(width: BigInt, value: BigInt) -> Result<ValueRef, SimError> {
    case_value(
        "value",
        fixed_number_mixop("S"),
        [
            make::nat(width, Region::none()),
            make::int(value, Region::none()),
        ],
    )
}

pub fn pack_p4_enum(type_id: &str, name: &str) -> Result<ValueRef, SimError> {
    case_value(
        "value",
        Mixfix::Seq(vec![Mixfix::Arg(()), operator("."), Mixfix::Arg(())]),
        [
            make::text(type_id.to_owned(), Region::none()),
            make::text(name.to_owned(), Region::none()),
        ],
    )
}

pub fn unpack_p4_enum(value: &ValueRef) -> Result<(String, String), SimError> {
    let arguments = case_arguments(
        value,
        Mixfix::Seq(vec![Mixfix::Arg(()), operator("."), Mixfix::Arg(())]),
    )?;
    let [type_id, name] = arguments.as_slice() else {
        return Err(SimError::message("expected a P4 enum"));
    };
    Ok((
        get::text(type_id).map(str::to_owned).map_err(value_error)?,
        get::text(name).map(str::to_owned).map_err(value_error)?,
    ))
}

pub fn pack_p4_tuple(values: Vec<ValueRef>) -> Result<ValueRef, SimError> {
    let value_type = named_type("value");
    let values = make::list(&make_type::list_type(value_type), values, Region::none());
    case_value(
        "value",
        Mixfix::Seq(vec![
            keyword("TUPLE"),
            Mixfix::Brack(
                Spanned::new(Atom::LParen, Region::none()),
                Box::new(Mixfix::Arg(())),
                Spanned::new(Atom::RParen, Region::none()),
            ),
        ]),
        [values],
    )
}

pub fn unpack_p4_tuple(value: &ValueRef) -> Result<Vec<ValueRef>, SimError> {
    let arguments = case_arguments(
        value,
        Mixfix::Seq(vec![
            keyword("TUPLE"),
            Mixfix::Brack(
                Spanned::new(Atom::LParen, Region::none()),
                Box::new(Mixfix::Arg(())),
                Spanned::new(Atom::RParen, Region::none()),
            ),
        ]),
    )?;
    let [values] = arguments.as_slice() else {
        return Err(SimError::message("expected a P4 tuple"));
    };
    get::list(values).map(<[_]>::to_vec).map_err(value_error)
}

pub fn unpack_p4_precision_number(value: &ValueRef) -> Result<(BigInt, BigInt), SimError> {
    unpack_p4_fixed_bit(value)
        .or_else(|_| unpack_p4_fixed_int(value))
        .or_else(|_| {
            let arguments = case_arguments(
                value,
                Mixfix::Seq(vec![
                    Mixfix::Arg(()),
                    operator("."),
                    Mixfix::Arg(()),
                    keyword("V"),
                    Mixfix::Arg(()),
                ]),
            )?;
            let [_maximum, width, value] = arguments.as_slice() else {
                return Err(SimError::message("expected a P4 precision number"));
            };
            Ok((integer(width)?, integer(value)?))
        })
}

pub fn unpack_p4_fixed_int(value: &ValueRef) -> Result<(BigInt, BigInt), SimError> {
    unpack_p4_fixed_number(value, "S")
}

pub fn pack_p4_string(value: &str) -> Result<ValueRef, SimError> {
    let quote = Atom::Operator("\"".to_owned());
    case_value(
        "value",
        Mixfix::Seq(vec![
            Mixfix::Atom(Spanned::new(quote.clone(), Region::none())),
            Mixfix::Arg(()),
            Mixfix::Atom(Spanned::new(quote, Region::none())),
        ]),
        [make::text(value.to_owned(), Region::none())],
    )
}

pub fn unpack_p4_string(value: &ValueRef) -> Result<String, SimError> {
    let quote = Atom::Operator("\"".to_owned());
    let arguments = case_arguments(
        value,
        Mixfix::Seq(vec![
            Mixfix::Atom(Spanned::new(quote.clone(), Region::none())),
            Mixfix::Arg(()),
            Mixfix::Atom(Spanned::new(quote, Region::none())),
        ]),
    )?;
    let [value] = arguments.as_slice() else {
        return Err(SimError::message("expected a P4 string"));
    };
    get::text(value).map(str::to_owned).map_err(value_error)
}

fn unpack_p4_fixed_number(value: &ValueRef, marker: &str) -> Result<(BigInt, BigInt), SimError> {
    let arguments = case_arguments(value, fixed_number_mixop(marker))?;
    let [width, value] = arguments.as_slice() else {
        return Err(SimError::message("expected a fixed-width P4 number"));
    };
    Ok((integer(width)?, integer(value)?))
}

fn integer(value: &ValueRef) -> Result<BigInt, SimError> {
    match get::num(value).map_err(value_error)? {
        num::T::Nat(value) | num::T::Int(value) => Ok(value.clone()),
    }
}

fn fixed_number_mixop(marker: &str) -> Mixop {
    Mixfix::Seq(vec![Mixfix::Arg(()), keyword(marker), Mixfix::Arg(())])
}

fn case_arguments(value: &ValueRef, expected: Mixop) -> Result<Vec<ValueRef>, SimError> {
    let value_case = get::case(value).map_err(value_error)?;
    if value_case.split().0 != expected {
        return Err(SimError::message("unexpected P4 value form"));
    }
    Ok(value_case.args().into_iter().cloned().collect())
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

fn operator(value: &str) -> Mixop {
    Mixfix::Atom(Spanned::new(
        Atom::Operator(value.to_owned()),
        Region::none(),
    ))
}

fn tag(value: &str) -> Mixop {
    Mixfix::Atom(Spanned::new(Atom::Tag(value.to_owned()), Region::none()))
}

fn value_error(error: impl ToString) -> SimError {
    SimError::message(error.to_string())
}

pub fn hex_to_bits(packet: &str) -> Result<Bits, SimError> {
    let mut bits = Vec::with_capacity(packet.len() * 4);
    for character in packet.chars() {
        let Some(nibble) = character.to_digit(16) else {
            return Err(SimError::message(format!(
                "invalid hexadecimal character `{character}`"
            )));
        };
        bits.extend([
            nibble & 8 != 0,
            nibble & 4 != 0,
            nibble & 2 != 0,
            nibble & 1 != 0,
        ]);
    }
    Ok(bits)
}

pub fn bits_to_hex(bits: &[bool]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut packet = String::with_capacity(bits.len().div_ceil(4));
    for nibble in bits.chunks(4) {
        let value = nibble
            .iter()
            .chain(std::iter::repeat(&false))
            .take(4)
            .fold(0, |value, bit| (value << 1) | usize::from(*bit));
        packet.push(char::from(DIGITS[value]));
    }
    packet
}

pub fn bits_to_unsigned(bits: &[bool]) -> BigInt {
    bits.iter().fold(BigInt::zero(), |value, bit| {
        (value << 1) + if *bit { BigInt::one() } else { BigInt::zero() }
    })
}

pub fn bits_to_signed(bits: &[bool]) -> Result<BigInt, SimError> {
    let Some(sign) = bits.first() else {
        return Err(SimError::message(
            "cannot decode an empty signed bit string",
        ));
    };
    let unsigned = bits_to_unsigned(bits);
    if *sign {
        Ok(unsigned - (BigInt::one() << bits.len()))
    } else {
        Ok(unsigned)
    }
}

pub fn unsigned_to_bits(value: &BigInt, width: usize) -> Bits {
    (0..width)
        .rev()
        .map(|index| ((value >> index) & BigInt::one()) > BigInt::zero())
        .collect()
}

pub fn signed_to_bits(value: &BigInt, width: usize) -> Bits {
    if width == 0 {
        return Vec::new();
    }
    let mask = (BigInt::one() << width) - BigInt::one();
    unsigned_to_bits(&(value & mask), width)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketIn {
    bits: Bits,
    cursor: usize,
}

impl PacketIn {
    pub fn new(packet: &str) -> Result<Self, SimError> {
        Ok(Self {
            bits: hex_to_bits(packet)?,
            cursor: 0,
        })
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn len_bits(&self) -> usize {
        self.bits.len()
    }

    pub fn len_bytes(&self) -> usize {
        self.bits.len().div_ceil(8)
    }

    pub fn take(&mut self, size: usize) -> Result<Bits, SimError> {
        let end = self
            .cursor
            .checked_add(size)
            .filter(|end| *end <= self.bits.len())
            .ok_or_else(|| SimError::message("PacketTooShort"))?;
        let bits = self.bits[self.cursor..end].to_vec();
        self.cursor = end;
        Ok(bits)
    }

    pub fn advance(&mut self, size: usize) -> Result<(), SimError> {
        self.take(size).map(|_| ())
    }

    pub fn payload_bits(&self) -> &[bool] {
        &self.bits[self.cursor..]
    }

    pub fn payload_hex(&self) -> String {
        bits_to_hex(self.payload_bits())
    }

    pub fn payload_bytes(&self) -> Vec<BigInt> {
        self.payload_bits()
            .chunks_exact(8)
            .map(bits_to_unsigned)
            .collect()
    }

    pub fn to_external(&self) -> ExternalData {
        ExternalData::Assoc(vec![
            (
                "kind".to_owned(),
                ExternalData::String("packet-in".to_owned()),
            ),
            (
                "bits".to_owned(),
                ExternalData::List(self.bits.iter().copied().map(ExternalData::Bool).collect()),
            ),
            (
                "cursor".to_owned(),
                ExternalData::Int(i64::try_from(self.cursor).unwrap_or(i64::MAX)),
            ),
        ])
    }

    pub fn from_external(value: &ExternalData) -> Result<Self, SimError> {
        let ExternalData::Assoc(fields) = value else {
            return Err(SimError::message("expected packet-in object state"));
        };
        let kind = external_field(fields, "kind")?;
        if kind != &ExternalData::String("packet-in".to_owned()) {
            return Err(SimError::message("expected packet-in object state"));
        }
        let ExternalData::List(bits) = external_field(fields, "bits")? else {
            return Err(SimError::message("packet-in bits must be a list"));
        };
        let bits = bits
            .iter()
            .map(|bit| match bit {
                ExternalData::Bool(bit) => Ok(*bit),
                _ => Err(SimError::message("packet-in bit must be a boolean")),
            })
            .collect::<Result<Bits, _>>()?;
        let ExternalData::Int(cursor) = external_field(fields, "cursor")? else {
            return Err(SimError::message("packet-in cursor must be an integer"));
        };
        let cursor = usize::try_from(*cursor)
            .ok()
            .filter(|cursor| *cursor <= bits.len())
            .ok_or_else(|| SimError::message("packet-in cursor is out of bounds"))?;
        Ok(Self { bits, cursor })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PacketOut {
    bits: Bits,
}

impl PacketOut {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, bits: &[bool]) {
        self.bits.extend_from_slice(bits);
    }

    pub fn bits(&self) -> &[bool] {
        &self.bits
    }

    pub fn packet_hex(&self, input: &PacketIn) -> String {
        let mut bits = self.bits.clone();
        bits.extend_from_slice(input.payload_bits());
        bits_to_hex(&bits)
    }

    pub fn to_external(&self) -> ExternalData {
        ExternalData::Assoc(vec![
            (
                "kind".to_owned(),
                ExternalData::String("packet-out".to_owned()),
            ),
            (
                "bits".to_owned(),
                ExternalData::List(self.bits.iter().copied().map(ExternalData::Bool).collect()),
            ),
        ])
    }

    pub fn from_external(value: &ExternalData) -> Result<Self, SimError> {
        let ExternalData::Assoc(fields) = value else {
            return Err(SimError::message("expected packet-out object state"));
        };
        let kind = external_field(fields, "kind")?;
        if kind != &ExternalData::String("packet-out".to_owned()) {
            return Err(SimError::message("expected packet-out object state"));
        }
        let ExternalData::List(bits) = external_field(fields, "bits")? else {
            return Err(SimError::message("packet-out bits must be a list"));
        };
        let bits = bits
            .iter()
            .map(|bit| match bit {
                ExternalData::Bool(bit) => Ok(*bit),
                _ => Err(SimError::message("packet-out bit must be a boolean")),
            })
            .collect::<Result<Bits, _>>()?;
        Ok(Self { bits })
    }
}

fn external_field<'a>(
    fields: &'a [(String, ExternalData)],
    name: &str,
) -> Result<&'a ExternalData, SimError> {
    fields
        .iter()
        .find_map(|(field, value)| (field == name).then_some(value))
        .ok_or_else(|| SimError::message(format!("missing external state field `{name}`")))
}
