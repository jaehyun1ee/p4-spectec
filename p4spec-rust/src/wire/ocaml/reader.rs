//! Serde reader for value envelopes with unique keys and integer numeric tokens
//!
//! Runtime Nat/Int values use decimal strings. Reject float tokens so oversized
//! integer tokens cannot silently enter the wire model through f64 rounding

use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Map;

use crate::util::json::json;

pub(super) fn from_slice(input: &[u8]) -> Result<json, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    deserializer.disable_recursion_limit();
    let json = JsonSeed.deserialize(serde_stacker::Deserializer::new(&mut deserializer))?;
    deserializer.end()?;
    Ok(json)
}

struct JsonSeed;

impl<'de> DeserializeSeed<'de> for JsonSeed {
    type Value = json;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<json, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for JsonSeed {
    type Value = json;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON with unique object keys and i64/u64 numeric tokens")
    }

    fn visit_unit<E: de::Error>(self) -> Result<json, E> {
        Ok(json::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<json, E> {
        Ok(json::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, int: i64) -> Result<json, E> {
        Ok(json::from(int))
    }

    fn visit_u64<E: de::Error>(self, int: u64) -> Result<json, E> {
        Ok(json::from(int))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<json, E> {
        Ok(json::String(text.to_owned()))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<json, E> {
        Ok(json::String(text))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<json, A::Error> {
        let mut jsons = Vec::new();
        while let Some(json) = seq.next_element_seed(JsonSeed)? {
            jsons.push(json);
        }
        Ok(json::Array(jsons))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<json, A::Error> {
        let mut fields = Map::new();
        while let Some(name) = map.next_key::<String>()? {
            if fields.contains_key(&name) {
                return Err(de::Error::custom(format!("duplicate JSON field `{name}`")));
            }
            fields.insert(name, map.next_value_seed(JsonSeed)?);
        }
        Ok(json::Object(fields))
    }
}
