pub mod architecture;
pub mod core;
pub mod ebpf;
pub mod hash;
pub mod io;
pub mod runner;
pub mod spec;
pub mod stf;

use thiserror::Error;

use crate::interface::ExternError;

#[derive(Debug, Error)]
pub enum SimError {
    #[error(transparent)]
    Extern(#[from] ExternError),
    #[error("{0}")]
    Message(String),
}

impl SimError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}
