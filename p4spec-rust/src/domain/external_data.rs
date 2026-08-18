//! Serialization-independent representation of `Yojson.Safe.t`

#[derive(Clone, Debug, PartialEq)]
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
