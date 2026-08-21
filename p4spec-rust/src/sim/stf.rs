use std::marker::PhantomData;

use crate::{interface::SpecCall, runtime::value::ValueRef, wire::sim_suite::StfStmt};

use super::{
    SimError,
    architecture::Architecture,
    io::{Expectation, PacketIo, Rx, Tx},
};

pub struct StfRunner<A> {
    context: ValueRef,
    architecture: ValueRef,
    io: PacketIo,
    matched: Vec<Tx>,
    marker: PhantomData<A>,
}

impl<A> StfRunner<A>
where
    A: Architecture,
{
    pub fn new(spec: &mut dyn SpecCall, program: &ValueRef) -> Result<Self, SimError> {
        let (context, architecture) = A::init(spec, program)?;
        Ok(Self {
            context,
            architecture,
            io: PacketIo::default(),
            matched: Vec::new(),
            marker: PhantomData,
        })
    }

    pub fn run_stmt(
        &mut self,
        spec: &mut dyn SpecCall,
        statement: StfStmt,
    ) -> Result<(), SimError> {
        match A::transform_stf(statement) {
            StfStmt::Wait => Ok(()),
            StfStmt::Packet { port, packet } => {
                let port = parse_port(&port)?;
                let (context, architecture, outputs) = A::drive(
                    spec,
                    self.context.clone(),
                    self.architecture.clone(),
                    Rx::new(port, packet),
                )?;
                self.context = context;
                self.architecture = architecture;
                self.matched.extend(self.io.push_outputs(outputs)?);
                Ok(())
            }
            StfStmt::Expect {
                port,
                packet,
                exact,
            } => {
                let expectation =
                    Expectation::new(parse_port(&port)?, packet.unwrap_or_default(), exact);
                if let Some(output) = self.io.push_expectation(expectation)? {
                    self.matched.push(output);
                }
                Ok(())
            }
            StfStmt::NoPacket if self.io.outputs().is_empty() => Ok(()),
            StfStmt::NoPacket => Err(SimError::message(format!(
                "expected no packet but got {}",
                self.io.outputs()[0]
            ))),
            statement => Err(SimError::message(format!(
                "not yet supported: {statement:?}"
            ))),
        }
    }

    pub fn finish(self) -> Result<Vec<Tx>, SimError> {
        self.io.finish()?;
        Ok(self.matched)
    }
}

fn parse_port(port: &str) -> Result<i32, SimError> {
    port.parse()
        .map_err(|_| SimError::message(format!("invalid port `{port}`")))
}
