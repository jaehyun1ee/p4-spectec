use num_bigint::BigInt;
use num_traits::{One, Zero};

use crate::domain::external_data::ExternalData;

use super::SimError;

pub type Bits = Vec<bool>;

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
