use serde_json::{Value, json};

use crate::domain::mixfix::{Mixfix, Mixop};

use super::{DecodeError, array, atom::AtomPhraseCodec, variant};

// Json serialization and deserialization

pub struct MixopCodec;

impl MixopCodec {
    pub fn decode(value: &Value) -> Result<Mixop, DecodeError> {
        let (tag, fields) = variant(value)?;
        match (tag, fields) {
            ("Arg", [unit]) if unit.is_null() => Ok(Mixfix::Arg(())),
            ("Atom", [atom]) => Ok(Mixfix::Atom(AtomPhraseCodec::decode(atom)?)),
            ("Brack", [left, body, right]) => Ok(Mixfix::Brack(
                AtomPhraseCodec::decode(left)?,
                Box::new(Self::decode(body)?),
                AtomPhraseCodec::decode(right)?,
            )),
            ("Infix", [left, atom, right]) => Ok(Mixfix::Infix(
                Box::new(Self::decode(left)?),
                AtomPhraseCodec::decode(atom)?,
                Box::new(Self::decode(right)?),
            )),
            ("Seq", [items]) => Ok(Mixfix::Seq(
                array(items)?
                    .iter()
                    .map(Self::decode)
                    .collect::<Result<_, _>>()?,
            )),
            ("Arg" | "Atom" | "Brack" | "Infix" | "Seq", _) => {
                Err(DecodeError::Expected("valid mixfix arity"))
            }
            (unknown, _) => Err(DecodeError::UnknownVariant(unknown.to_owned())),
        }
    }

    pub fn encode(mixop: &Mixop) -> Value {
        match mixop {
            Mixfix::Arg(()) => json!(["Arg", null]),
            Mixfix::Atom(atom) => json!(["Atom", AtomPhraseCodec::encode(atom)]),
            Mixfix::Brack(left, body, right) => json!([
                "Brack",
                AtomPhraseCodec::encode(left),
                Self::encode(body),
                AtomPhraseCodec::encode(right)
            ]),
            Mixfix::Infix(left, atom, right) => json!([
                "Infix",
                Self::encode(left),
                AtomPhraseCodec::encode(atom),
                Self::encode(right)
            ]),
            Mixfix::Seq(items) => {
                json!(["Seq", items.iter().map(Self::encode).collect::<Vec<_>>()])
            }
        }
    }
}
