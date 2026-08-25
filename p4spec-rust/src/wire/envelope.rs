use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

pub const EL_SCHEMA: &str = "p4spectec.el.v1";
pub const IL_SCHEMA: &str = "p4spectec.il.v1";
pub const AL_SCHEMA: &str = "p4spectec.al.v1";
pub const SL_SCHEMA: &str = "p4spectec.sl.v1";
pub const PL_SCHEMA: &str = "p4spectec.pl.v1";
pub const VALUE_SCHEMA: &str = "p4spectec.value.v1";

const EL_KIND: &str = "el";
const IL_KIND: &str = "il";
const AL_KIND: &str = "al";
const SL_KIND: &str = "sl";
const PL_KIND: &str = "pl";

/// A versioned JSON payload for one language stage
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Envelope<T> {
    schema: String,
    kind: String,
    payload: T,
}

impl<T> Envelope<T> {
    /// Builds an EL envelope
    pub fn el(payload: T) -> Self {
        Self::new(EL_SCHEMA, EL_KIND, payload)
    }

    /// Builds an IL envelope
    pub fn il(payload: T) -> Self {
        Self::new(IL_SCHEMA, IL_KIND, payload)
    }

    /// Builds an AL envelope
    pub fn al(payload: T) -> Self {
        Self::new(AL_SCHEMA, AL_KIND, payload)
    }

    /// Builds an SL envelope
    pub fn sl(payload: T) -> Self {
        Self::new(SL_SCHEMA, SL_KIND, payload)
    }

    /// Builds a PL envelope
    pub fn pl(payload: T) -> Self {
        Self::new(PL_SCHEMA, PL_KIND, payload)
    }

    /// Borrows the versioned schema name
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Borrows the stage kind name
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Borrows the typed payload
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Returns the typed payload
    pub fn into_payload(self) -> T {
        self.payload
    }

    fn new(schema: &str, kind: &str, payload: T) -> Self {
        Self {
            schema: schema.to_owned(),
            kind: kind.to_owned(),
            payload,
        }
    }

    fn validate(&self) -> Result<(), WireError> {
        let expected_kind = match self.schema.as_str() {
            EL_SCHEMA => EL_KIND,
            IL_SCHEMA => IL_KIND,
            AL_SCHEMA => AL_KIND,
            SL_SCHEMA => SL_KIND,
            PL_SCHEMA => PL_KIND,
            schema => return Err(WireError::UnknownSchema(schema.to_owned())),
        };

        if self.kind != expected_kind {
            return Err(WireError::SchemaKindMismatch {
                schema: self.schema.clone(),
                kind: self.kind.clone(),
                expected_kind,
            });
        }

        Ok(())
    }
}

impl<T> Envelope<T>
where
    T: DeserializeOwned,
{
    /// Decodes one complete JSON envelope
    ///
    /// Rejects unknown schemas;
    /// rejects schema-kind mismatches and trailing JSON
    pub fn from_slice(input: &[u8]) -> Result<Self, WireError> {
        let mut deserializer = serde_json::Deserializer::from_slice(input);
        deserializer.disable_recursion_limit();
        let envelope = {
            let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
            Self::deserialize(deserializer)?
        };
        deserializer.end()?;
        envelope.validate()?;
        Ok(envelope)
    }
}

#[derive(Debug, Error)]
pub enum WireError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unknown wire schema `{0}`")]
    UnknownSchema(String),

    #[error("schema `{schema}` requires kind `{expected_kind}`, but the envelope uses `{kind}`")]
    SchemaKindMismatch {
        schema: String,
        kind: String,
        expected_kind: &'static str,
    },
}
