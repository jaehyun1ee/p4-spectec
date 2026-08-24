use serde_json::{Value, json};

use crate::domain::source::{Position, Region, Spanned};

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
) -> Result<Spanned<T>, DecodeError> {
    let object = object(value)?;
    if !field(object, "note")?.is_null() {
        return Err(DecodeError::Expected("null unit note"));
    }
    Ok(Spanned::new(
        decode_it(field(object, "it")?)?,
        decode_region(field(object, "at")?)?,
    ))
}

pub fn encode_phrase<T>(phrase: &Spanned<T>, encode_it: impl FnOnce(&T) -> Value) -> Value {
    json!({
        "it": encode_it(&phrase.node),
        "note": null,
        "at": encode_region(&phrase.span),
    })
}

pub(crate) fn decode_annotated<T, N>(
    value: &Value,
    decode_it: impl FnOnce(&Value) -> Result<T, DecodeError>,
    decode_note: impl FnOnce(&Value) -> Result<N, DecodeError>,
) -> Result<(T, N, Region), DecodeError> {
    let object = object(value)?;
    Ok((
        decode_it(field(object, "it")?)?,
        decode_note(field(object, "note")?)?,
        decode_region(field(object, "at")?)?,
    ))
}

pub(crate) fn encode_annotated<T, N>(
    node: &T,
    note: &N,
    span: &Region,
    encode_it: impl FnOnce(&T) -> Value,
    encode_note: impl FnOnce(&N) -> Value,
) -> Value {
    json!({"it": encode_it(node), "note": encode_note(note), "at": encode_region(span)})
}
