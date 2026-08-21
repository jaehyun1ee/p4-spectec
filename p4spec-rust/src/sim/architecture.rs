use crate::{
    interface::{Extern, SpecCall},
    runtime::value::ValueRef,
    wire::sim_suite::StfStmt,
};

use super::{
    SimError,
    io::{Rx, Tx},
};

pub trait Architecture: Extern + Sized {
    fn name() -> &'static str;

    fn init(spec: &mut dyn SpecCall, program: &ValueRef) -> Result<(ValueRef, ValueRef), SimError>;

    fn drive(
        spec: &mut dyn SpecCall,
        context: ValueRef,
        architecture: ValueRef,
        rx: Rx,
    ) -> Result<(ValueRef, ValueRef, Vec<Tx>), SimError>;

    fn transform_stf(stmt: StfStmt) -> StfStmt {
        stmt
    }
}
