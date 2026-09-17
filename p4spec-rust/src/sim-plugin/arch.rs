//! Architecture operations used by native STF execution

use super::{io::Rx, state::SimState};
use crate::{
    lang::data::value::Value,
    runner::{Extern, ExternError, Interface, Interpreter, RunnerContext},
    stf::ast::Statement,
};
use num_bigint::BigInt;

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
        rx: &Rx,
    ) -> Result<(), Interp::Error>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>;

    fn add_mirror_session<Interp, Iface>(
        _ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        _value_arch: Value,
        _session: usize,
        _port: usize,
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
        _session: usize,
        _group: usize,
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
        _group: usize,
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
        _instance: usize,
        _ports: &[usize],
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
        _group: usize,
        _handle: usize,
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
        _idx: usize,
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
        _idx: usize,
        _int: BigInt,
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

macro_rules! delegate_pipe {
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
            rx: &Rx,
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

macro_rules! delegate_method {
    ($pipe:path, $name:ident $(, $arg:ident: $typ:ty)*) => {
        fn $name<Interp, Iface>(
            ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
            value_arch: Value,
            $($arg: $typ),*
        ) -> Result<Value, Interp::Error>
        where
            Iface: Interface,
            Interp: Interpreter<Iface, Self>,
        {
            use $pipe as pipe;
            pipe::$name(ctx, value_arch, $($arg),*)
        }
    };
}

impl Architecture for super::ebpf::Ebpf {
    const NAME: &'static str = "ebpf";

    delegate_pipe!(super::ebpf::pipe);
}

impl Architecture for super::psa::Psa {
    const NAME: &'static str = "psa";

    delegate_pipe!(super::psa::pipe);

    delegate_method!(super::psa::pipe, add_mirror_session_mc, session: usize, group: usize);
    delegate_method!(super::psa::pipe, mc_mgrp_create, group: usize);
    delegate_method!(super::psa::pipe, mc_node_create, instance: usize, ports: &[usize]);
    delegate_method!(super::psa::pipe, mc_node_associate, group: usize, handle: usize);
    delegate_method!(super::psa::pipe, register_read, name: &str, idx: usize);
    delegate_method!(super::psa::pipe, register_write, name: &str, idx: usize, int: BigInt);
    delegate_method!(super::psa::pipe, register_reset, name: &str);
}

impl Architecture for super::v1model::V1Model {
    const NAME: &'static str = "v1model";

    delegate_pipe!(super::v1model::pipe);

    delegate_method!(super::v1model::pipe, add_mirror_session, session: usize, port: usize);
    delegate_method!(super::v1model::pipe, add_mirror_session_mc, session: usize, group: usize);
    delegate_method!(super::v1model::pipe, mc_mgrp_create, group: usize);
    delegate_method!(super::v1model::pipe, mc_node_create, instance: usize, ports: &[usize]);
    delegate_method!(super::v1model::pipe, mc_node_associate, group: usize, handle: usize);
    delegate_method!(super::v1model::pipe, register_read, name: &str, idx: usize);
    delegate_method!(super::v1model::pipe, register_write, name: &str, idx: usize, int: BigInt);
    delegate_method!(super::v1model::pipe, register_reset, name: &str);
}
