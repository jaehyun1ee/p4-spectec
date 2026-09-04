//! Serialization-independent representation of extended JSON data

use std::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug)]
pub enum ExternalData {
    /// JSON null
    Null,
    /// JSON boolean
    Bool(bool),
    /// JSON number without a decimal point or exponent
    Int(i64),
    /// Arbitrary integer preserved as a decimal string
    Intlit(String),
    /// JSON number, Infinity, -Infinity, or NaN
    Float(f64),
    /// JSON string
    String(String),
    /// JSON object, preserving field order and duplicate names
    Assoc(Vec<(String, Self)>),
    /// JSON array
    List(Vec<Self>),
    /// Tuple, a non-standard JSON extension
    Tuple(Vec<Self>),
    /// Variant, a non-standard JSON extension
    Variant(String, Option<Box<Self>>),
}

// == Comparison

fn rank(value: &ExternalData) -> u8 {
    match value {
        ExternalData::Null => 0,
        ExternalData::String(_) => 1,
        ExternalData::Intlit(_) => 2,
        ExternalData::Int(_) => 3,
        ExternalData::Float(_) => 4,
        ExternalData::Variant(_, _) => 5,
        ExternalData::Tuple(_) => 6,
        ExternalData::Bool(_) => 7,
        ExternalData::List(_) => 8,
        ExternalData::Assoc(_) => 9,
    }
}

fn compare_float(float_l: f64, float_r: f64) -> Ordering {
    match (float_l.is_nan(), float_r.is_nan()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => float_l
            .partial_cmp(&float_r)
            .expect("non-NaN floats have a total order"),
    }
}

impl Ord for ExternalData {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Null, Self::Null) => Ordering::Equal,
            (Self::String(value_l), Self::String(value_r))
            | (Self::Intlit(value_l), Self::Intlit(value_r)) => value_l.cmp(value_r),
            (Self::Int(value_l), Self::Int(value_r)) => value_l.cmp(value_r),
            (Self::Float(value_l), Self::Float(value_r)) => compare_float(*value_l, *value_r),
            (Self::Variant(name_l, value_l), Self::Variant(name_r, value_r)) => {
                name_l.cmp(name_r).then_with(|| value_l.cmp(value_r))
            }
            (Self::Tuple(values_l), Self::Tuple(values_r))
            | (Self::List(values_l), Self::List(values_r)) => values_l.cmp(values_r),
            (Self::Bool(value_l), Self::Bool(value_r)) => value_l.cmp(value_r),
            (Self::Assoc(fields_l), Self::Assoc(fields_r)) => fields_l.cmp(fields_r),
            _ => rank(self).cmp(&rank(other)),
        }
    }
}

impl PartialOrd for ExternalData {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ExternalData {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ExternalData {}

// == Hashing

impl Hash for ExternalData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        rank(self).hash(state);
        match self {
            Self::Null => {}
            Self::Bool(value) => value.hash(state),
            Self::Int(value) => value.hash(state),
            Self::Intlit(value) | Self::String(value) => value.hash(state),
            Self::Float(value) => {
                let bits = if value.is_nan() {
                    f64::NAN.to_bits()
                } else if *value == 0.0 {
                    0.0f64.to_bits()
                } else {
                    value.to_bits()
                };
                bits.hash(state);
            }
            Self::Assoc(fields) => fields.hash(state),
            Self::List(values) | Self::Tuple(values) => values.hash(state),
            Self::Variant(name, value) => {
                name.hash(state);
                value.hash(state);
            }
        }
    }
}
