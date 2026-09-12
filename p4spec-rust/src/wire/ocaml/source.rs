use serde_json::json;

use crate::util::json::json;

use crate::lang::common::source::{NotePhrase, Phrase, Position, Span};

use super::{DecodeError, field, integer, object};

pub fn decode_position(json: &json) -> Result<Position, DecodeError> {
    let object = object(json)?;
    Ok(Position::new(
        field(object, "file")?
            .as_str()
            .ok_or(DecodeError::Expected("position file string"))?,
        integer(field(object, "line")?)?,
        integer(field(object, "column")?)?,
    ))
}

pub fn encode_position(position: &Position) -> json {
    json!({
        "file": position.file.as_ref(),
        "line": position.line,
        "column": position.column,
    })
}

pub fn decode_region(json: &json) -> Result<Span, DecodeError> {
    let object = object(json)?;
    Ok(Span::new(
        decode_position(field(object, "left")?)?,
        decode_position(field(object, "right")?)?,
    ))
}

pub fn encode_region(region: &Span) -> json {
    json!({
        "left": encode_position(&region.left),
        "right": encode_position(&region.right),
    })
}

pub fn decode_phrase<T>(
    json: &json,
    decode_it: impl FnOnce(&json) -> Result<T, DecodeError>,
) -> Result<Phrase<T>, DecodeError> {
    decode_note_phrase(json, decode_it, |json| {
        if json.is_null() {
            Ok(())
        } else {
            Err(DecodeError::Expected("null unit note"))
        }
    })
}

pub fn encode_phrase<T>(phrase: &Phrase<T>, encode_it: impl FnOnce(&T) -> json) -> json {
    encode_note_phrase(phrase, encode_it, |_| json::Null)
}

pub(crate) fn decode_note_phrase<T, N>(
    json: &json,
    decode_it: impl FnOnce(&json) -> Result<T, DecodeError>,
    decode_note: impl FnOnce(&json) -> Result<N, DecodeError>,
) -> Result<NotePhrase<T, N>, DecodeError> {
    let object = object(json)?;
    Ok(crate::note_phrase! {
        node: decode_it(field(object, "it")?)?,
        note: decode_note(field(object, "note")?)?,
        span: decode_region(field(object, "at")?)?,
    })
}

pub(crate) fn encode_note_phrase<T, N>(
    phrase: &NotePhrase<T, N>,
    encode_it: impl FnOnce(&T) -> json,
    encode_note: impl FnOnce(&N) -> json,
) -> json {
    json!({
        "it": encode_it(&phrase.node),
        "note": encode_note(&phrase.note),
        "at": encode_region(&phrase.span),
    })
}
