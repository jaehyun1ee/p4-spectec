use super::super::state::SimState;
use super::pipe;
use crate::runner::{Extern, Interface, Interpreter, RunnerContext};

/// Schedule queued packets in FIFO order
pub fn run_scheduler<Interp, Iface, Exn>(
    ctx: &mut RunnerContext<'_, Interp, Iface, Exn>,
    state: &mut SimState,
) -> Result<(), Interp::Error>
where
    Iface: Interface,
    Exn: Extern,
    Interp: Interpreter<Iface, Exn>,
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
