//! Extern relations of the two towers
//!
//! `PacketEngine` serves `Parse_pkt` to the mini P4 specification under
//! meta-execution.
//! `Relay` serves `ExtRel_call` to the mini p4st-in-p4st specification under
//! meta-meta-execution: it decodes the call, answers it as
//! `PacketEngine` does, and encodes the reply.

use std::rc::Rc;

use p4spectec::{
    lang::{
        data::value::{flat::Value, tree::ValueKind},
        il::ast::Typ,
    },
    runner::{Eval, Extern, ExternError, Interface, Interpreter, RunnerContext, SideEffect},
};

use crate::{
    encode::Encoder,
    engine,
    value::{self as v, Error, Registry, typ_var},
};

fn fail<T>(error: Error) -> Eval<T, ExternError> {
    Eval::new(Err(ExternError::diagnostic_message(error)), SideEffect::Pure)
}

/// Parses `pkt` into `hdr` as the extern relation `Parse_pkt`.
fn parse_pkt(p4: &Registry, inputs: Vec<v::Tree>) -> Result<Vec<v::Tree>, Error> {
    let [pkt, hdr] = <[_; 2]>::try_from(inputs).map_err(|_| "Parse_pkt takes pkt and p4Val")?;
    let (pkt, hdr) = engine::parse_pkt(p4, pkt, hdr)?;
    Ok(vec![pkt, hdr])
}

// = Meta-execution

/// The Packet Engine as seen by the mini P4 specification.
pub struct PacketEngine {
    pub p4: Rc<Registry>,
}

impl Extern for PacketEngine {
    fn eval_rel<Interp, Iface>(
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        values: &[Value],
    ) -> Eval<Vec<Value>, ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let (arena, external) = ctx.arena_and_external_mut();
        if name != "Parse_pkt" {
            return fail(format!("the Packet Engine has no relation {name}"));
        }
        let result = parse_pkt(&external.p4, v::read_many(arena, values)).and_then(|outputs| {
            outputs
                .into_iter()
                .map(|output| v::intern(arena, output))
                .collect()
        });
        // Parsing is a function of its inputs, so a call may be cached
        match result {
            Ok(outputs) => Eval::new(Ok(outputs), SideEffect::Pure),
            Err(error) => fail(error),
        }
    }

    fn eval_func<Interp, Iface>(
        _: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        _: &[Typ],
        _: &[Value],
    ) -> Eval<Value, ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        fail(format!("the Packet Engine has no function {name}"))
    }

    fn clear(&mut self) {}
}

// = Meta-meta-execution

/// `ExtRel_call` of p4st-in-p4st, relaying calls of the mini P4 specification.
pub struct Relay {
    pub p4: Rc<Registry>,
    pub st: Rc<Registry>,
}

impl Relay {
    /// Answers `ExtRel_call: |- id val* : vsRes` with `OK val_o*`.
    fn call(&self, inputs: Vec<v::Tree>) -> Result<v::Tree, Error> {
        let [id, vals] = <[_; 2]>::try_from(inputs).map_err(|_| "ExtRel_call takes id and val*")?;
        let (ValueKind::Text(id), ValueKind::List(vals)) = (id.node, vals.node) else {
            return Err("ExtRel_call takes a text and a list".to_owned());
        };
        if id != "Parse_pkt" {
            return Err(format!("the Packet Engine has no relation {id}"));
        }
        // Decode the call, answer it as under meta-execution, and encode the reply
        let inputs = vals
            .into_iter()
            .map(|val| Encoder::unval(&self.p4, val))
            .collect::<Result<_, _>>()?;
        let outputs = parse_pkt(&self.p4, inputs)?;
        let encoder = Encoder::new(&self.st);
        let vals = outputs
            .into_iter()
            .map(|output| encoder.val(output))
            .collect::<Result<_, _>>()?;
        self.st
            .case_as(typ_var("vsRes"), "OK %", vec![v::list(typ_var("val"), vals)])
    }
}

impl Extern for Relay {
    fn eval_rel<Interp, Iface>(
        ctx: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        values: &[Value],
    ) -> Eval<Vec<Value>, ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        let (arena, external) = ctx.arena_and_external_mut();
        if name != "ExtRel_call" {
            return fail(format!("p4st-in-p4st has no extern relation {name}"));
        }
        let result = external
            .call(v::read_many(arena, values))
            .and_then(|output| v::intern(arena, output));
        match result {
            Ok(output) => Eval::new(Ok(vec![output]), SideEffect::Pure),
            Err(error) => fail(error),
        }
    }

    fn eval_func<Interp, Iface>(
        _: &mut RunnerContext<'_, Interp, Iface, Self>,
        name: &str,
        _: &[Typ],
        _: &[Value],
    ) -> Eval<Value, ExternError>
    where
        Iface: Interface,
        Interp: Interpreter<Iface, Self>,
    {
        fail(format!("p4st-in-p4st has no extern function {name}"))
    }

    fn clear(&mut self) {}
}
