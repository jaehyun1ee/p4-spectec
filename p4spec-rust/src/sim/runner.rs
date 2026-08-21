use std::marker::PhantomData;

use crate::{interface::SpecCall, runtime::value::ValueRef, wire::sim_suite::StfStmt};

use super::{SimError, architecture::Architecture, io::Tx, stf::StfRunner};

pub struct SuiteRunner<A>(PhantomData<A>);

impl<A> SuiteRunner<A>
where
    A: Architecture,
{
    pub fn run_case(
        spec: &mut dyn SpecCall,
        program: &ValueRef,
        statements: &[StfStmt],
    ) -> Result<Vec<Tx>, SimError> {
        let mut runner = StfRunner::<A>::new(spec, program)?;
        for statement in statements {
            runner.run_stmt(spec, statement.clone())?;
        }
        runner.finish()
    }
}
