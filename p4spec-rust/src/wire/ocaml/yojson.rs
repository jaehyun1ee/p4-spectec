//! Lossless wire representation of `Yojson.Safe.t`

use std::{str, str::Utf8Error};

use serde_json::{Map, Number};
use thiserror::Error;

use super::on_codec_stack;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Intlit(String),
    Float(f64),
    String(String),
    Assoc(Vec<(String, Self)>),
    List(Vec<Self>),
    Tuple(Vec<Self>),
    Variant(String, Option<Box<Self>>),
}

impl Value {
    pub fn from_slice(input: &[u8]) -> Result<Self, ParseError> {
        on_codec_stack(|| Parser::new(input)?.parse())
    }

    pub fn to_vec(&self) -> Result<Vec<u8>, WriteError> {
        on_codec_stack(|| {
            let mut output = Vec::new();
            write_value(&mut output, self)?;
            Ok(output)
        })
    }
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Yojson input is not UTF-8: {0}")]
    Utf8(#[from] Utf8Error),

    #[error("invalid Yojson at byte {offset}: expected {expected}")]
    Syntax {
        offset: usize,
        expected: &'static str,
    },
}

#[derive(Debug, Error)]
pub enum WriteError {
    #[error("Yojson Int `{0}` is outside the OCaml machine-integer range")]
    IntOutsideRange(i64),

    #[error("Yojson Intlit `{0}` is inside the OCaml machine-integer range")]
    IntlitInsideRange(String),

    #[error("invalid Yojson integer literal `{0}`")]
    InvalidIntlit(String),

    #[error("cannot encode Yojson string: {0}")]
    String(#[from] serde_json::Error),
}

struct Parser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Result<Self, ParseError> {
        Ok(Self {
            input: str::from_utf8(input)?,
            bytes: input,
            offset: 0,
        })
    }

    fn parse(mut self) -> Result<Value, ParseError> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.offset != self.bytes.len() {
            return self.error("end of input");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            Some(b'n') => {
                self.consume(b"null", "`null`")?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.consume(b"true", "`true`")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.consume(b"false", "`false`")?;
                Ok(Value::Bool(false))
            }
            Some(b'N') => {
                self.consume(b"NaN", "`NaN`")?;
                Ok(Value::Float(f64::NAN))
            }
            Some(b'I') => {
                self.consume(b"Infinity", "`Infinity`")?;
                Ok(Value::Float(f64::INFINITY))
            }
            Some(b'-') if self.remaining().starts_with(b"-Infinity") => {
                self.consume(b"-Infinity", "`-Infinity`")?;
                Ok(Value::Float(f64::NEG_INFINITY))
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b'{') => self.parse_assoc(),
            Some(b'[') => self.parse_list(),
            Some(b'(') => self.parse_tuple(),
            Some(b'<') => self.parse_variant(),
            Some(_) => self.error("Yojson value"),
            None => self.error("Yojson value"),
        }
    }

    fn parse_number(&mut self) -> Result<Value, ParseError> {
        let start = self.offset;
        if self.peek() == Some(b'-') {
            self.offset += 1;
        }

        match self.peek() {
            Some(b'0') => self.offset += 1,
            Some(b'1'..=b'9') => {
                self.offset += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.offset += 1;
                }
            }
            _ => return self.error("integer digits"),
        }

        let mut float = false;
        if self.peek() == Some(b'.') {
            float = true;
            self.offset += 1;
            let digits = self.offset;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
            if self.offset == digits {
                return self.error("fraction digits");
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            float = true;
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            let digits = self.offset;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
            if self.offset == digits {
                return self.error("exponent digits");
            }
        }

        let literal = &self.input[start..self.offset];
        if float {
            literal
                .parse::<f64>()
                .map(Value::Float)
                .map_err(|_| ParseError::Syntax {
                    offset: start,
                    expected: "finite or non-finite float",
                })
        } else {
            match literal.parse::<i64>() {
                Ok(value) if ocaml_int_contains(value) => Ok(Value::Int(value)),
                _ => Ok(Value::Intlit(literal.to_owned())),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        self.expect(b'"', "string")?;
        let mut output = String::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.offset += 1;
                    return Ok(output);
                }
                Some(b'\\') => {
                    self.offset += 1;
                    self.parse_escape(&mut output)?;
                }
                Some(0..=0x1f) => return self.error("escaped control character"),
                Some(byte) if byte.is_ascii() => {
                    output.push(char::from(byte));
                    self.offset += 1;
                }
                Some(_) => {
                    let Some(character) = self.input[self.offset..].chars().next() else {
                        return self.error("UTF-8 character");
                    };
                    output.push(character);
                    self.offset += character.len_utf8();
                }
                None => return self.error("closing string quote"),
            }
        }
    }

    fn parse_escape(&mut self, output: &mut String) -> Result<(), ParseError> {
        match self.next() {
            Some(b'"') => output.push('"'),
            Some(b'\\') => output.push('\\'),
            Some(b'/') => output.push('/'),
            Some(b'b') => output.push('\u{0008}'),
            Some(b'f') => output.push('\u{000c}'),
            Some(b'n') => output.push('\n'),
            Some(b'r') => output.push('\r'),
            Some(b't') => output.push('\t'),
            Some(b'u') => {
                let high = self.parse_hex_quad()?;
                let scalar = if (0xd800..=0xdbff).contains(&high) {
                    self.consume(b"\\u", "low-surrogate escape")?;
                    let low = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return self.error("low surrogate");
                    }
                    0x10000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(low) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&high) {
                    return self.error("high surrogate");
                } else {
                    u32::from(high)
                };
                let Some(character) = char::from_u32(scalar) else {
                    return self.error("Unicode scalar value");
                };
                output.push(character);
            }
            Some(_) => return self.error("valid string escape"),
            None => return self.error("string escape"),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, ParseError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = match self.next() {
                Some(b'0'..=b'9') => u16::from(self.bytes[self.offset - 1] - b'0'),
                Some(b'a'..=b'f') => u16::from(self.bytes[self.offset - 1] - b'a' + 10),
                Some(b'A'..=b'F') => u16::from(self.bytes[self.offset - 1] - b'A' + 10),
                _ => return self.error("four hexadecimal escape digits"),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_assoc(&mut self) -> Result<Value, ParseError> {
        self.expect(b'{', "object")?;
        self.skip_whitespace();
        let mut fields = Vec::new();
        if self.peek() == Some(b'}') {
            self.offset += 1;
            return Ok(Value::Assoc(fields));
        }

        loop {
            let name = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':', "object field colon")?;
            self.skip_whitespace();
            let value = self.parse_value()?;
            fields.push((name, value));
            self.skip_whitespace();
            match self.next() {
                Some(b',') => self.skip_whitespace(),
                Some(b'}') => return Ok(Value::Assoc(fields)),
                _ => return self.error("object comma or closing brace"),
            }
        }
    }

    fn parse_list(&mut self) -> Result<Value, ParseError> {
        self.expect(b'[', "list")?;
        self.parse_sequence(b']').map(Value::List)
    }

    fn parse_tuple(&mut self) -> Result<Value, ParseError> {
        self.expect(b'(', "tuple")?;
        self.parse_sequence(b')').map(Value::Tuple)
    }

    fn parse_sequence(&mut self, end: u8) -> Result<Vec<Value>, ParseError> {
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.peek() == Some(end) {
            self.offset += 1;
            return Ok(values);
        }

        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            match self.next() {
                Some(b',') => self.skip_whitespace(),
                Some(byte) if byte == end => return Ok(values),
                _ => return self.error("sequence comma or closing delimiter"),
            }
        }
    }

    fn parse_variant(&mut self) -> Result<Value, ParseError> {
        self.expect(b'<', "variant")?;
        self.skip_whitespace();
        let name = self.parse_string()?;
        self.skip_whitespace();
        match self.next() {
            Some(b'>') => Ok(Value::Variant(name, None)),
            Some(b':') => {
                self.skip_whitespace();
                let value = self.parse_value()?;
                self.skip_whitespace();
                self.expect(b'>', "variant closing bracket")?;
                Ok(Value::Variant(name, Some(Box::new(value))))
            }
            _ => self.error("variant colon or closing bracket"),
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.offset += 1;
        }
    }

    fn consume(&mut self, expected: &[u8], name: &'static str) -> Result<(), ParseError> {
        if self.remaining().starts_with(expected) {
            self.offset += expected.len();
            Ok(())
        } else {
            self.error(name)
        }
    }

    fn expect(&mut self, expected: u8, name: &'static str) -> Result<(), ParseError> {
        if self.next() == Some(expected) {
            Ok(())
        } else {
            self.error(name)
        }
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.offset += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn remaining(&self) -> &[u8] {
        &self.bytes[self.offset..]
    }

    fn error<T>(&self, expected: &'static str) -> Result<T, ParseError> {
        Err(ParseError::Syntax {
            offset: self.offset,
            expected,
        })
    }
}

fn ocaml_int_contains(value: i64) -> bool {
    let payload_bits = usize::BITS - 1;
    let minimum = -(1_i64 << (payload_bits - 1));
    let maximum = (1_i64 << (payload_bits - 1)) - 1;
    (minimum..=maximum).contains(&value)
}

fn valid_intlit(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    match digits.as_bytes() {
        [b'0'] => true,
        [b'1'..=b'9', rest @ ..] => rest.iter().all(u8::is_ascii_digit),
        _ => false,
    }
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<(), WriteError> {
    output.extend(serde_json::to_vec(value)?);
    Ok(())
}

fn write_values(
    output: &mut Vec<u8>,
    values: &[Value],
    open: u8,
    close: u8,
) -> Result<(), WriteError> {
    output.push(open);
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(b',');
        }
        write_value(output, value)?;
    }
    output.push(close);
    Ok(())
}

fn write_value(output: &mut Vec<u8>, value: &Value) -> Result<(), WriteError> {
    match value {
        Value::Null => output.extend(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Int(value) if ocaml_int_contains(*value) => output.extend(value.to_string().bytes()),
        Value::Int(value) => return Err(WriteError::IntOutsideRange(*value)),
        Value::Intlit(value)
            if valid_intlit(value) && value.parse::<i64>().is_ok_and(ocaml_int_contains) =>
        {
            return Err(WriteError::IntlitInsideRange(value.clone()));
        }
        Value::Intlit(value) if valid_intlit(value) => output.extend(value.bytes()),
        Value::Intlit(value) => return Err(WriteError::InvalidIntlit(value.clone())),
        Value::Float(value) if value.is_nan() => output.extend(b"NaN"),
        Value::Float(value) if *value == f64::INFINITY => output.extend(b"Infinity"),
        Value::Float(value) if *value == f64::NEG_INFINITY => output.extend(b"-Infinity"),
        Value::Float(value) => {
            let literal = value.to_string();
            output.extend(literal.bytes());
            if !literal.contains(['.', 'e', 'E']) {
                output.extend(b".0");
            }
        }
        Value::String(value) => write_string(output, value)?,
        Value::Assoc(fields) => {
            output.push(b'{');
            for (index, (name, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_string(output, name)?;
                output.push(b':');
                write_value(output, value)?;
            }
            output.push(b'}');
        }
        Value::List(values) => write_values(output, values, b'[', b']')?,
        Value::Tuple(values) => write_values(output, values, b'(', b')')?,
        Value::Variant(name, value) => {
            output.push(b'<');
            write_string(output, name)?;
            if let Some(value) = value {
                output.push(b':');
                write_value(output, value)?;
            }
            output.push(b'>');
        }
    }
    Ok(())
}

pub(crate) fn from_serde_json(value: &serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                if ocaml_int_contains(value) {
                    Value::Int(value)
                } else {
                    Value::Intlit(value.to_string())
                }
            } else if value.is_u64() {
                Value::Intlit(value.to_string())
            } else if let Some(value) = value.as_f64() {
                Value::Float(value)
            } else {
                Value::Intlit(value.to_string())
            }
        }
        serde_json::Value::String(value) => Value::String(value.clone()),
        serde_json::Value::Array(values) => {
            Value::List(values.iter().map(from_serde_json).collect())
        }
        serde_json::Value::Object(fields) => Value::Assoc(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), from_serde_json(value)))
                .collect(),
        ),
    }
}

pub(crate) fn to_serde_json(value: &Value) -> Result<serde_json::Value, &'static str> {
    match value {
        Value::Null => Ok(serde_json::Value::Null),
        Value::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        Value::Int(value) => Ok(serde_json::Value::Number(Number::from(*value))),
        Value::Intlit(_) => Err("standard JSON value without arbitrary integer"),
        Value::Float(value) => Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .ok_or("finite standard JSON float"),
        Value::String(value) => Ok(serde_json::Value::String(value.clone())),
        Value::Assoc(fields) => {
            let mut object = Map::new();
            for (name, value) in fields {
                if object.contains_key(name) {
                    return Err("standard JSON object without duplicate fields");
                }
                object.insert(name.clone(), to_serde_json(value)?);
            }
            Ok(serde_json::Value::Object(object))
        }
        Value::List(values) => values
            .iter()
            .map(to_serde_json)
            .collect::<Result<_, _>>()
            .map(serde_json::Value::Array),
        Value::Tuple(_) => Err("standard JSON value without tuples"),
        Value::Variant(_, _) => Err("standard JSON value without variants"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, WriteError};

    #[test]
    fn writer_preserves_ocaml_int_constructor_boundaries() {
        let payload_bits = usize::BITS - 1;
        let minimum = -(1_i64 << (payload_bits - 1));
        let maximum = (1_i64 << (payload_bits - 1)) - 1;

        for value in [minimum, maximum] {
            let bytes = Value::Int(value).to_vec().expect("encode OCaml Int");
            assert_eq!(
                Value::from_slice(&bytes).expect("reparse Int"),
                Value::Int(value)
            );
        }

        for value in [minimum - 1, maximum + 1] {
            let error = Value::Int(value)
                .to_vec()
                .expect_err("reject out-of-range Int");
            assert!(matches!(error, WriteError::IntOutsideRange(actual) if actual == value));

            let literal = value.to_string();
            let bytes = Value::Intlit(literal.clone())
                .to_vec()
                .expect("encode out-of-range Intlit");
            assert_eq!(
                Value::from_slice(&bytes).expect("reparse Intlit"),
                Value::Intlit(literal),
            );
        }
    }

    #[test]
    fn writer_rejects_intlit_inside_ocaml_int_range() {
        for literal in ["-1", "0", "1"] {
            let error = Value::Intlit(literal.to_owned())
                .to_vec()
                .expect_err("reject in-range Intlit");
            assert!(matches!(error, WriteError::IntlitInsideRange(actual) if actual == literal));
        }
    }
}
