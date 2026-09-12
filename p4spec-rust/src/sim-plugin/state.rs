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

// Both relation result types carry these fields; keep the call result available
macro_rules! install_result {
    ($state:ident, $result:ident $(,)?) => {{
        $state.value_ctx = $result.value_ctx;
        $state.value_arch = $result.value_arch;
    }};
}

pub(super) use install_result;
