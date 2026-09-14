use super::super::state::SimState;
use super::{Psa, pipe};
use crate::runner::{Interface, Interpreter, RunnerContext};

/// Schedule queued packets in FIFO order
pub fn run_scheduler<Interp, Iface>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Psa>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Interp: Interpreter<Iface, Psa>,
{
    loop {
        let mut arch = pipe::get_arch_state(ctx, state.value_arch)?;
        let Some(packet) = arch.queue.pop_front() else {
            return Ok(());
        };
        state.value_arch = pipe::set_arch_state(ctx, state.value_arch, &arch)?;
        pipe::drive_packet(ctx, state, packet)?;
    }
}
