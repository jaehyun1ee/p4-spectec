//! Total syntax order consistent with serde JSON equality and hashing

use std::cmp::Ordering;

use serde_json::{Number, Value};

fn rank(value: &Value) -> u8 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn compare_num(num_l: &Number, num_r: &Number) -> Ordering {
    let int = |num: &Number| {
        num.as_i64()
            .map(i128::from)
            .or_else(|| num.as_u64().map(i128::from))
    };
    match (int(num_l), int(num_r)) {
        (Some(int_l), Some(int_r)) => int_l.cmp(&int_r),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => num_l
            .as_f64()
            .expect("finite JSON float")
            .partial_cmp(&num_r.as_f64().expect("finite JSON float"))
            .expect("finite JSON floats have a total order"),
    }
}

pub(super) fn compare(value_l: &Value, value_r: &Value) -> Ordering {
    match (value_l, value_r) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Bool(value_l), Value::Bool(value_r)) => value_l.cmp(value_r),
        (Value::Number(num_l), Value::Number(num_r)) => compare_num(num_l, num_r),
        (Value::String(text_l), Value::String(text_r)) => text_l.cmp(text_r),
        (Value::Array(values_l), Value::Array(values_r)) => values_l
            .iter()
            .zip(values_r)
            .map(|(value_l, value_r)| compare(value_l, value_r))
            .find(|order| !order.is_eq())
            .unwrap_or_else(|| values_l.len().cmp(&values_r.len())),
        (Value::Object(fields_l), Value::Object(fields_r)) => {
            // Also preserve key-order independence if serde's preserve_order is enabled
            let mut fields_l = fields_l.iter().collect::<Vec<_>>();
            let mut fields_r = fields_r.iter().collect::<Vec<_>>();
            fields_l.sort_unstable_by_key(|(name, _)| *name);
            fields_r.sort_unstable_by_key(|(name, _)| *name);
            fields_l
                .iter()
                .zip(&fields_r)
                .map(|((name_l, value_l), (name_r, value_r))| {
                    name_l.cmp(name_r).then_with(|| compare(value_l, value_r))
                })
                .find(|order| !order.is_eq())
                .unwrap_or_else(|| fields_l.len().cmp(&fields_r.len()))
        }
        _ => rank(value_l).cmp(&rank(value_r)),
    }
}
