//! Serialization-independent representation of external JSON-compatible data.

#[derive(Clone, Debug, PartialEq)]
pub enum ExternalData {
    Null,
    Bool(bool),
    Int(i64),
    Intlit(String),
    Float(f64),
    String(String),
    Assoc(Vec<(String, Self)>),
    List(Vec<Self>),
    Tuple(Vec<Self>),
    Variant(String, Option<Box<Self>>),
}
