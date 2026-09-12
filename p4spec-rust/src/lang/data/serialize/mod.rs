//! Arena-aware native value payloads and shared JSON annotation codecs

pub mod atom;
pub mod mixfix;
pub(crate) mod num;
pub mod source;
pub(crate) mod typ;
pub mod value;

use serde_json::Map;
use thiserror::Error;

use crate::util::json::json;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error(transparent)]
    Value(#[from] crate::lang::data::value::ValueError),
    #[error("expected {0}")]
    Expected(&'static str),
    #[error("missing field `{0}`")]
    MissingField(&'static str),
    #[error("unknown OCaml variant `{0}`")]
    UnknownVariant(String),
}

const CODEC_STACK_SIZE: usize = 32 * 1024 * 1024;

pub(crate) fn on_codec_stack<T>(codec: impl FnOnce() -> T) -> T {
    stacker::grow(CODEC_STACK_SIZE, codec)
}

pub(crate) fn array(json: &json) -> Result<&[json], DecodeError> {
    json.as_array()
        .map(Vec::as_slice)
        .ok_or(DecodeError::Expected("array"))
}

pub(crate) fn string(json: &json) -> Result<&str, DecodeError> {
    json.as_str().ok_or(DecodeError::Expected("string"))
}

pub(crate) fn boolean(json: &json) -> Result<bool, DecodeError> {
    json.as_bool().ok_or(DecodeError::Expected("boolean"))
}

pub(crate) fn integer(json: &json) -> Result<i64, DecodeError> {
    json.as_i64().ok_or(DecodeError::Expected("integer"))
}

pub(crate) fn object(json: &json) -> Result<&Map<String, json>, DecodeError> {
    json.as_object().ok_or(DecodeError::Expected("object"))
}

pub(crate) fn field<'a>(
    fields: &'a Map<String, json>,
    name: &'static str,
) -> Result<&'a json, DecodeError> {
    fields.get(name).ok_or(DecodeError::MissingField(name))
}

pub(crate) fn variant(json: &json) -> Result<(&str, &[json]), DecodeError> {
    let jsons = array(json)?;
    let (tag, fields) = jsons
        .split_first()
        .ok_or(DecodeError::Expected("non-empty variant array"))?;
    Ok((string(tag)?, fields))
}

pub(crate) fn decode_list<T>(
    json: &json,
    decode: impl FnMut(&json) -> Result<T, DecodeError>,
) -> Result<Vec<T>, DecodeError> {
    array(json)?.iter().map(decode).collect()
}

pub(crate) fn encode_list<T>(values: &[T], encode: impl Fn(&T) -> json) -> json {
    json::Array(values.iter().map(encode).collect())
}
