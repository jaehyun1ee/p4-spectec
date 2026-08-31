use serde_json::{Value, json};

use crate::lang::common::source::{NotePhrase, Phrase, Position, Span};

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
        "file": position.file.as_ref(),
        "line": position.line,
        "column": position.column,
    })
}

pub fn decode_region(value: &Value) -> Result<Span, DecodeError> {
    let object = object(value)?;
    Ok(Span::new(
        decode_position(field(object, "left")?)?,
        decode_position(field(object, "right")?)?,
    ))
}

pub fn encode_region(region: &Span) -> Value {
    json!({
        "left": encode_position(&region.left),
        "right": encode_position(&region.right),
    })
}

pub fn decode_phrase<T>(
    value: &Value,
    decode_it: impl FnOnce(&Value) -> Result<T, DecodeError>,
) -> Result<Phrase<T>, DecodeError> {
    decode_note_phrase(value, decode_it, |value| {
        if value.is_null() {
            Ok(())
        } else {
            Err(DecodeError::Expected("null unit note"))
        }
    })
}

pub fn encode_phrase<T>(phrase: &Phrase<T>, encode_it: impl FnOnce(&T) -> Value) -> Value {
    encode_note_phrase(phrase, encode_it, |_| Value::Null)
}

pub(crate) fn decode_note_phrase<T, N>(
    value: &Value,
    decode_it: impl FnOnce(&Value) -> Result<T, DecodeError>,
    decode_note: impl FnOnce(&Value) -> Result<N, DecodeError>,
) -> Result<NotePhrase<T, N>, DecodeError> {
    let object = object(value)?;
    Ok(crate::note_phrase! {
        node: decode_it(field(object, "it")?)?,
        note: decode_note(field(object, "note")?)?,
        span: decode_region(field(object, "at")?)?,
    })
}

pub(crate) fn encode_note_phrase<T, N>(
    phrase: &NotePhrase<T, N>,
    encode_it: impl FnOnce(&T) -> Value,
    encode_note: impl FnOnce(&N) -> Value,
) -> Value {
    json!({
        "it": encode_it(&phrase.node),
        "note": encode_note(&phrase.note),
        "at": encode_region(&phrase.span),
    })
}
