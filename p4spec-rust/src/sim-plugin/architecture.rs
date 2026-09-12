//! Architecture operations used by native STF execution

use super::{io::Transmission, state::SimState};
use crate::{
    lang::data::value::Value,
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
};

pub trait Architecture: Extern {
    const NAME: &'static str;
    fn transform_stf_stmt(stmt: Statement) -> Statement;
    fn init_pipe<Interp, Iface>(
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        program: Value,
    ) -> Result<SimState, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;
    fn drive_pipe<Interp, Iface>(
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        state: &mut SimState,
        rx: &Transmission,
    ) -> Result<(), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;
    fn add_mirror_session<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _session: i64,
        _port: i64,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "add_mirror_session is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn add_mirror_session_mc<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _session: i64,
        _group: i64,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "add_mirror_session_mc is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn mc_mgrp_create<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _group: i64,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "mc_mgrp_create is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn mc_node_create<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _instance: i64,
        _ports: &[i64],
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "mc_node_create is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn mc_node_associate<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _group: i64,
        _handle: i64,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "mc_node_associate is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn register_read<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _name: &str,
        _idx: i64,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "register_read is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn register_write<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _name: &str,
        _idx: i64,
        _int: i64,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "register_write is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
    fn register_reset<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _name: &str,
    ) -> Result<Value, Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        Err(ExternError::Failure(format!(
            "register_reset is not implemented for the {} simulator",
            Self::NAME
        ))
        .into())
    }
}

macro_rules! pipe_ops {
    ($pipe:path) => {
        fn transform_stf_stmt(stmt: Statement) -> Statement {
            use $pipe as pipe;
            pipe::transform_stf_stmt(stmt)
        }
        fn init_pipe<Interp, Iface>(
            ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
            program: Value,
        ) -> Result<SimState, Interp::Error>
        where
            Iface: Interface,
            Interp: Interpreter<Iface, Self>,
        {
            use $pipe as pipe;
            pipe::init_pipe(ctx, program)
        }
        fn drive_pipe<Interp, Iface>(
            ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
            state: &mut SimState,
            rx: &Transmission,
        ) -> Result<(), Interp::Error>
        where
            Iface: Interface,
            Interp: Interpreter<Iface, Self>,
        {
            use $pipe as pipe;
            pipe::drive_pipe(ctx, state, rx)
        }
    };
}

macro_rules! forward_op {
    ($pipe:path, $name:ident $(, $arg:ident: $typ:ty)*) => {
        fn $name<Interp, Iface>(ctx: &mut RunnerContext<'_, Interp, Iface, Self>, value_arch: Value, $($arg: $typ),*) -> Result<Value, Interp::Error>
        where Iface: Interface, Interp: Interpreter<Iface, Self> {
            use $pipe as pipe;
            pipe::$name(ctx, value_arch, $($arg),*)
        }
    };
}

impl Architecture for super::ebpf::Ebpf {
    const NAME: &'static str = "ebpf";
    pipe_ops!(super::ebpf::pipe);
}

impl Architecture for super::psa::Psa {
    const NAME: &'static str = "psa";
    pipe_ops!(super::psa::pipe);
    forward_op!(super::psa::pipe, add_mirror_session_mc, session: i64, group: i64);
    forward_op!(super::psa::pipe, mc_mgrp_create, group: i64);
    forward_op!(super::psa::pipe, mc_node_create, instance: i64, ports: &[i64]);
    forward_op!(super::psa::pipe, mc_node_associate, group: i64, handle: i64);
    forward_op!(super::psa::pipe, register_read, name: &str, idx: i64);
    forward_op!(super::psa::pipe, register_write, name: &str, idx: i64, int: i64);
    forward_op!(super::psa::pipe, register_reset, name: &str);
}

impl Architecture for super::v1model::V1Model {
    const NAME: &'static str = "v1model";
    pipe_ops!(super::v1model::pipe);
    forward_op!(super::v1model::pipe, add_mirror_session, session: i64, port: i64);
    forward_op!(super::v1model::pipe, add_mirror_session_mc, session: i64, group: i64);
    forward_op!(super::v1model::pipe, mc_mgrp_create, group: i64);
    forward_op!(super::v1model::pipe, mc_node_create, instance: i64, ports: &[i64]);
    forward_op!(super::v1model::pipe, mc_node_associate, group: i64, handle: i64);
    forward_op!(super::v1model::pipe, register_read, name: &str, idx: i64);
    forward_op!(super::v1model::pipe, register_write, name: &str, idx: i64, int: i64);
    forward_op!(super::v1model::pipe, register_reset, name: &str);
}
