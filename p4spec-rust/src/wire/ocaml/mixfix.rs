use serde_json::{Value, json};

use crate::lang::common::notation::{mixfix::Mixfix, mixop::Mixop};

use super::{DecodeError, array, atom::AtomPhraseCodec, on_codec_stack, variant};

// Json serialization and deserialization

pub struct MixopCodec;

impl MixopCodec {
    pub fn decode(value: &Value) -> Result<Mixop, DecodeError> {
        on_codec_stack(|| {
            decode(value, |unit| {
                if unit.is_null() {
                    Ok(())
                } else {
                    Err(DecodeError::Expected("null unit argument"))
                }
            })
        })
    }

    pub fn encode(mixop: &Mixop) -> Value {
        on_codec_stack(|| encode(mixop, |()| Value::Null))
    }
}

pub(crate) fn try_encode<T, E>(
    mixfix: &Mixfix<T>,
    encode_arg: impl Copy + Fn(&T) -> Result<Value, E>,
) -> Result<Value, E> {
    Ok(match mixfix {
        Mixfix::Arg(arg) => json!(["Arg", encode_arg(arg)?]),
        Mixfix::Atom(atom) => json!(["Atom", AtomPhraseCodec::encode(atom)]),
        Mixfix::Brack(left, body, right) => json!([
            "Brack",
            AtomPhraseCodec::encode(left),
            try_encode(body, encode_arg)?,
            AtomPhraseCodec::encode(right)
        ]),
        Mixfix::Infix(left, atom, right) => json!([
            "Infix",
            try_encode(left, encode_arg)?,
            AtomPhraseCodec::encode(atom),
            try_encode(right, encode_arg)?
        ]),
        Mixfix::Seq(items) => json!([
            "Seq",
            items
                .iter()
                .map(|item| try_encode(item, encode_arg))
                .collect::<Result<Vec<_>, _>>()?
        ]),
    })
}

pub(crate) fn decode<T>(
    value: &Value,
    decode_arg: impl Copy + Fn(&Value) -> Result<T, DecodeError>,
) -> Result<Mixfix<T>, DecodeError> {
    let (tag, fields) = variant(value)?;
    match (tag, fields) {
        ("Arg", [arg]) => Ok(Mixfix::Arg(decode_arg(arg)?)),
        ("Atom", [atom]) => Ok(Mixfix::Atom(AtomPhraseCodec::decode(atom)?)),
        ("Brack", [left, body, right]) => Ok(Mixfix::Brack(
            AtomPhraseCodec::decode(left)?,
            Box::new(decode(body, decode_arg)?),
            AtomPhraseCodec::decode(right)?,
        )),
        ("Infix", [left, atom, right]) => Ok(Mixfix::Infix(
            Box::new(decode(left, decode_arg)?),
            AtomPhraseCodec::decode(atom)?,
            Box::new(decode(right, decode_arg)?),
        )),
        ("Seq", [items]) => Ok(Mixfix::Seq(
            array(items)?
                .iter()
                .map(|item| decode(item, decode_arg))
                .collect::<Result<_, _>>()?,
        )),
        ("Arg" | "Atom" | "Brack" | "Infix" | "Seq", _) => {
            Err(DecodeError::Expected("valid mixfix arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(crate) fn encode<T>(mixfix: &Mixfix<T>, encode_arg: impl Copy + Fn(&T) -> Value) -> Value {
    match mixfix {
        Mixfix::Arg(arg) => json!(["Arg", encode_arg(arg)]),
        Mixfix::Atom(atom) => json!(["Atom", AtomPhraseCodec::encode(atom)]),
        Mixfix::Brack(left, body, right) => json!([
            "Brack",
            AtomPhraseCodec::encode(left),
            encode(body, encode_arg),
            AtomPhraseCodec::encode(right)
        ]),
        Mixfix::Infix(left, atom, right) => json!([
            "Infix",
            encode(left, encode_arg),
            AtomPhraseCodec::encode(atom),
            encode(right, encode_arg)
        ]),
        Mixfix::Seq(items) => {
            json!([
                "Seq",
                items
                    .iter()
                    .map(|item| encode(item, encode_arg))
                    .collect::<Vec<_>>()
            ])
        }
    }
}
