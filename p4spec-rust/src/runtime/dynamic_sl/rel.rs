use std::fmt;

use crate::lang::sl::ast::{Block, ElseBlock, Exp, RelSignature};

// Relation

#[derive(Clone, Debug, PartialEq)]
pub enum Relation {
    Extern(RelSignature),
    Defined(RelSignature, Vec<Exp>, Block, Option<ElseBlock>),
}

impl fmt::Display for Relation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Extern(_) => "extern relation",
            Self::Defined(..) => "defined relation",
        })
    }
}

impl Relation {
    pub fn get_signature(&self) -> &RelSignature {
        match self {
            Self::Extern(signature) | Self::Defined(signature, ..) => signature,
        }
    }
}
