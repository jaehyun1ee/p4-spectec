use serde_json::json;

use crate::util::json::json;

use crate::lang::common::notation::{mixfix::Mixfix, mixop::Mixop};

use super::{DecodeError, array, atom::AtomPhraseCodec, on_codec_stack, variant};

// Json serialization and deserialization

pub struct MixopCodec;

impl MixopCodec {
    pub fn decode(json: &json) -> Result<Mixop, DecodeError> {
        on_codec_stack(|| {
            decode(json, |unit| {
                if unit.is_null() {
                    Ok(())
                } else {
                    Err(DecodeError::Expected("null unit argument"))
                }
            })
        })
    }

    pub fn encode(mixop: &Mixop) -> json {
        on_codec_stack(|| encode(mixop, |()| json::Null))
    }
}

pub(crate) fn try_encode<T, E>(
    mixfix: &Mixfix<T>,
    encode_arg: impl Copy + Fn(&T) -> Result<json, E>,
) -> Result<json, E> {
    Ok(match mixfix {
        Mixfix::Arg(arg) => json!(["Arg", encode_arg(arg)?]),
        Mixfix::Atom(atom) => json!(["Atom", AtomPhraseCodec::encode(atom)]),
        Mixfix::Brack(atom_l, mixfix_inner, atom_r) => json!([
            "Brack",
            AtomPhraseCodec::encode(atom_l),
            try_encode(mixfix_inner, encode_arg)?,
            AtomPhraseCodec::encode(atom_r)
        ]),
        Mixfix::Infix(mixfix_l, atom, mixfix_r) => json!([
            "Infix",
            try_encode(mixfix_l, encode_arg)?,
            AtomPhraseCodec::encode(atom),
            try_encode(mixfix_r, encode_arg)?
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
    json: &json,
    mut decode_arg: impl FnMut(&json) -> Result<T, DecodeError>,
) -> Result<Mixfix<T>, DecodeError> {
    decode_inner(json, &mut decode_arg)
}

fn decode_inner<T>(
    json: &json,
    decode_arg: &mut impl FnMut(&json) -> Result<T, DecodeError>,
) -> Result<Mixfix<T>, DecodeError> {
    let (tag, fields) = variant(json)?;
    match (tag, fields) {
        ("Arg", [arg]) => Ok(Mixfix::Arg(decode_arg(arg)?)),
        ("Atom", [atom]) => Ok(Mixfix::Atom(AtomPhraseCodec::decode(atom)?)),
        ("Brack", [atom_l, mixfix_inner, atom_r]) => Ok(Mixfix::Brack(
            AtomPhraseCodec::decode(atom_l)?,
            Box::new(decode_inner(mixfix_inner, decode_arg)?),
            AtomPhraseCodec::decode(atom_r)?,
        )),
        ("Infix", [mixfix_l, atom, mixfix_r]) => Ok(Mixfix::Infix(
            Box::new(decode_inner(mixfix_l, decode_arg)?),
            AtomPhraseCodec::decode(atom)?,
            Box::new(decode_inner(mixfix_r, decode_arg)?),
        )),
        ("Seq", [items]) => Ok(Mixfix::Seq(
            array(items)?
                .iter()
                .map(|item| decode_inner(item, decode_arg))
                .collect::<Result<_, _>>()?,
        )),
        ("Arg" | "Atom" | "Brack" | "Infix" | "Seq", _) => {
            Err(DecodeError::Expected("valid mixfix arity"))
        }
        (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
    }
}

pub(crate) fn encode<T>(mixfix: &Mixfix<T>, encode_arg: impl Copy + Fn(&T) -> json) -> json {
    match mixfix {
        Mixfix::Arg(arg) => json!(["Arg", encode_arg(arg)]),
        Mixfix::Atom(atom) => json!(["Atom", AtomPhraseCodec::encode(atom)]),
        Mixfix::Brack(atom_l, mixfix_inner, atom_r) => json!([
            "Brack",
            AtomPhraseCodec::encode(atom_l),
            encode(mixfix_inner, encode_arg),
            AtomPhraseCodec::encode(atom_r)
        ]),
        Mixfix::Infix(mixfix_l, atom, mixfix_r) => json!([
            "Infix",
            encode(mixfix_l, encode_arg),
            AtomPhraseCodec::encode(atom),
            encode(mixfix_r, encode_arg)
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
