use serde_json::{Value, json};

use crate::domain::source::{Info, Phrase, Position, Region};

use super::{DecodeError, field, integer, object};

pub fn decode_position(value: &Value) -> Result<Position, DecodeError> {
    let object = object(value)?;
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
    let object = object(value)?;
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
    let object = object(value)?;
    if !field(object, "note")?.is_null() {
        return Err(DecodeError::Expected("null unit note"));
    }
    Ok(Info::new(
        decode_it(field(object, "it")?)?,
        decode_region(field(object, "at")?)?,
    ))
}

pub(crate) fn decode_note_phrase<T, N>(
    value: &Value,
    decode_it: impl FnOnce(&Value) -> Result<T, DecodeError>,
    decode_note: impl FnOnce(&Value) -> Result<N, DecodeError>,
) -> Result<Info<T, N, Region>, DecodeError> {
    let object = object(value)?;
    Ok(Info::with_note(
        decode_it(field(object, "it")?)?,
        decode_region(field(object, "at")?)?,
        decode_note(field(object, "note")?)?,
    ))
}

pub(crate) fn encode_note_phrase<T, N>(
    phrase: &Info<T, N, Region>,
    encode_it: impl FnOnce(&T) -> Value,
    encode_note: impl FnOnce(&N) -> Value,
) -> Value {
    json!({
        "it": encode_it(&phrase.it),
        "note": encode_note(&phrase.note),
        "at": encode_region(&phrase.at),
    })
}

pub fn encode_phrase<T>(phrase: &Phrase<T>, encode_it: impl FnOnce(&T) -> Value) -> Value {
    json!({
        "it": encode_it(&phrase.it),
        "note": null,
        "at": encode_region(&phrase.at),
    })
}
