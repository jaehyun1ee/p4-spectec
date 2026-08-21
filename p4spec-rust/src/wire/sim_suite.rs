use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::runtime::value::ValueRef;

use super::{
    Envelope, SIM_SUITE_SCHEMA, WireError,
    ocaml::lang::il::{ValueEnvelopeCodec, ValueEnvelopeDecodeError},
    runtime_value,
};

#[derive(Clone, Debug)]
pub struct SimSuite {
    pub arch: String,
    pub entries: Vec<SimEntry>,
}

#[derive(Clone, Debug)]
pub enum SimEntry {
    Run {
        p4_path: String,
        stf_path: String,
        patched: bool,
        program: ValueRef,
        stf: Vec<StfStmt>,
    },
    Exclude {
        p4_path: String,
        stf_path: String,
        patched: bool,
        group: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StfStmt {
    Wait,
    RemoveAll,
    Expect {
        port: String,
        packet: Option<String>,
        exact: bool,
    },
    Packet {
        port: String,
        packet: String,
    },
    NoPacket,
    Add {
        name: String,
        priority: Option<i32>,
        matches: Vec<StfMatch>,
        action: StfAction,
        id: Option<String>,
    },
    SetDefault {
        name: String,
        action: StfAction,
    },
    CheckCounter {
        id: String,
        id_or_index: StfIdOrIndex,
        counter: Option<StfCounter>,
        condition: StfCondition,
        number: String,
    },
    MirroringAdd {
        session: String,
        port: String,
    },
    MirroringAddMc {
        session: String,
        id: String,
    },
    MirroringGet {
        session: String,
    },
    McGroupCreate {
        id: String,
    },
    McNodeCreate {
        id: String,
        ports: Vec<String>,
    },
    McNodeAssociate {
        id: String,
        handle: String,
    },
    RegisterRead {
        name: String,
        index: String,
    },
    RegisterWrite {
        name: String,
        index: String,
        value: String,
    },
    RegisterReset {
        name: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct StfAction {
    pub name: String,
    pub args: Vec<StfActionArg>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct StfActionArg {
    pub id: String,
    pub number: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct StfMatch {
    pub name: String,
    pub value: StfMatchValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StfMatchValue {
    Num { value: String },
    Slash { prefix: String, mask: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum StfIdOrIndex {
    Id(String),
    Index(String),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StfCondition {
    Eq,
    Ne,
    Le,
    Lt,
    Ge,
    Gt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StfCounter {
    Bytes,
    Packets,
}

#[derive(Debug, Deserialize)]
struct SimSuitePayload {
    arch: String,
    entries: Vec<SimEntryWire>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SimEntryWire {
    Run {
        p4_path: String,
        stf_path: String,
        patched: bool,
        program: Value,
        stf: Vec<StfStmt>,
    },
    Exclude {
        p4_path: String,
        stf_path: String,
        patched: bool,
        group: Option<String>,
    },
}

pub struct SimSuiteCodec;

impl SimSuiteCodec {
    pub fn decode(input: &[u8]) -> Result<SimSuite, SimSuiteDecodeError> {
        let envelope = Envelope::<Value>::from_slice(input)?;
        if envelope.schema() != SIM_SUITE_SCHEMA {
            return Err(SimSuiteDecodeError::ExpectedSchema(
                envelope.schema().to_owned(),
            ));
        }
        let payload: SimSuitePayload = serde_json::from_value(envelope.into_payload())?;
        if !matches!(payload.arch.as_str(), "ebpf" | "psa" | "v1model") {
            return Err(SimSuiteDecodeError::UnknownArchitecture(payload.arch));
        }
        let entries = payload
            .entries
            .into_iter()
            .map(decode_entry)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SimSuite {
            arch: payload.arch,
            entries,
        })
    }
}

fn decode_entry(entry: SimEntryWire) -> Result<SimEntry, SimSuiteDecodeError> {
    Ok(match entry {
        SimEntryWire::Run {
            p4_path,
            stf_path,
            patched,
            program,
            stf,
        } => {
            let bytes = serde_json::to_vec(&program)?;
            let program = ValueEnvelopeCodec::decode(&bytes)?;
            SimEntry::Run {
                p4_path,
                stf_path,
                patched,
                program: runtime_value::to_runtime(&program),
                stf,
            }
        }
        SimEntryWire::Exclude {
            p4_path,
            stf_path,
            patched,
            group,
        } => SimEntry::Exclude {
            p4_path,
            stf_path,
            patched,
            group,
        },
    })
}

#[derive(Debug, Error)]
pub enum SimSuiteDecodeError {
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Value(#[from] ValueEnvelopeDecodeError),
    #[error("expected schema `{SIM_SUITE_SCHEMA}`, got `{0}`")]
    ExpectedSchema(String),
    #[error("unknown simulation architecture `{0}`")]
    UnknownArchitecture(String),
}
