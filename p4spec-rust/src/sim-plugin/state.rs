use crate::lang::data::value::Value;

use super::io::Transmission;

/// Mutable pipeline state, held outside the runner
///
/// Values belong to the accompanying runner's arena. `value_arch` includes
/// architecture work queues; `txs` holds one input packet's outputs in order
pub struct SimState {
    pub value_ctx: Value,
    pub value_arch: Value,
    pub txs: Vec<Transmission>,
}
