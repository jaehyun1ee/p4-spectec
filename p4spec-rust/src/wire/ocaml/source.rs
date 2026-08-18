use serde_json::{Map, Value, json};

use crate::domain::source::{Info, Phrase, Position, Region};

use super::DecodeError;

fn field<'a>(object: &'a Map<String, Value>, name: &'static str) -> Result<&'a Value, DecodeError> {
    object.get(name).ok_or(DecodeError::MissingField(name))
}

fn integer(value: &Value) -> Result<i64, DecodeError> {
    value.as_i64().ok_or(DecodeError::Expected("integer"))
}

pub fn decode_position(value: &Value) -> Result<Position, DecodeError> {
    let object = value
        .as_object()
        .ok_or(DecodeError::Expected("position object"))?;
    Ok(Position::new(
        field(object, "file")?
            .as_str()
            .ok_or(DecodeError::Expected("position file string"))?,
        integer(field(object, "line")?)?,
        integer(field(object, "column")?)?,
    ))
}

pub fn encode_position(position: &Position) -> Value {
    json!({
        "file": position.file,
        "line": position.line,
        "column": position.column,
    })
}

pub fn decode_region(value: &Value) -> Result<Region, DecodeError> {
    let object = value
        .as_object()
        .ok_or(DecodeError::Expected("region object"))?;
    Ok(Region::new(
        decode_position(field(object, "left")?)?,
        decode_position(field(object, "right")?)?,
    ))
}

pub fn encode_region(region: &Region) -> Value {
    json!({
        "left": encode_position(&region.left),
        "right": encode_position(&region.right),
    })
}

pub fn decode_phrase<T>(
    value: &Value,
    decode_it: impl FnOnce(&Value) -> Result<T, DecodeError>,
) -> Result<Phrase<T>, DecodeError> {
    let object = value
        .as_object()
        .ok_or(DecodeError::Expected("phrase object"))?;
    if !field(object, "note")?.is_null() {
        return Err(DecodeError::Expected("null unit note"));
    }
    Ok(Info::new(
        decode_it(field(object, "it")?)?,
        decode_region(field(object, "at")?)?,
    ))
}

pub fn encode_phrase<T>(phrase: &Phrase<T>, encode_it: impl FnOnce(&T) -> Value) -> Value {
    json!({
        "it": encode_it(&phrase.it),
        "note": null,
        "at": encode_region(&phrase.at),
    })
}
