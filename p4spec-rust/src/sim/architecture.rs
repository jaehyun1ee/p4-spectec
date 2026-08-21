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

    fn add_mirror_session(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _session: i32,
        _port: i32,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message("mirror session is not supported"))
    }

    fn add_mirror_session_mc(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _session: i32,
        _multicast_group: i32,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message(
            "multicast mirror session is not supported",
        ))
    }

    fn mc_group_create(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _group: i32,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message("multicast groups are not supported"))
    }

    fn mc_node_create(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _instance: i32,
        _ports: Vec<i32>,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message("multicast nodes are not supported"))
    }

    fn mc_node_associate(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _group: i32,
        _handle: i32,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message(
            "multicast node association is not supported",
        ))
    }

    fn register_read(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _name: &str,
        _index: i32,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message("register read is not supported"))
    }

    fn register_write(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _name: &str,
        _index: i32,
        _value: i32,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message("register write is not supported"))
    }

    fn register_reset(
        _spec: &mut dyn SpecCall,
        _architecture: ValueRef,
        _name: &str,
    ) -> Result<ValueRef, SimError> {
        Err(SimError::message("register reset is not supported"))
    }
}
