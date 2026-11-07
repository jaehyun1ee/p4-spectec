//! The Packet Engine of the simplified eBPF architecture
//!
//! A packet `pkt` is an extern meta-value holding its remaining bytes.
//! `parse_pkt` serves the extern relation `Parse_pkt`:
//! it fills each field of a header, one byte per field, in order.
//! `Simulator::run` initiates the pipeline on one packet:
//! it calls `P4Block_call` on `prs`, then on `flt`, through a tower evaluator.

use p4spectec::lang::data::{
    arena::Arena,
    value::{
        flat,
        tree::{Value, ValueKind},
    },
};

use crate::value::{self as v, Error, Registry, typ_var};

/// Fills the header `hdr` from the bytes of `pkt`; returns the rest of the packet and the header.
pub fn parse_pkt(p4: &Registry, pkt: Value, hdr: Value) -> Result<(Value, Value), Error> {
    // Read the remaining bytes of the packet
    let ValueKind::Extern(json) = pkt.node else { return Err("pkt is an extern value".to_owned()) };
    let mut bytes = json
        .as_array()
        .ok_or("pkt holds its bytes")?
        .iter()
        .map(|byte| byte.as_u64().ok_or("a byte is a number"))
        .collect::<Result<std::collections::VecDeque<_>, _>>()?;
    // Fill each field of the header with the next byte
    let (ctor, mut args) = v::uncase(hdr)?;
    if ctor != "HdrV % %" || args.len() != 2 {
        return Err(format!("Parse_pkt expects a header, not `{ctor}`"));
    }
    let fields = args.pop().expect("HdrV has fields");
    let name = args.pop().expect("HdrV has a name");
    let fields_note = fields.note.clone();
    let ValueKind::List(fields) = fields.node else { return Err("header fields".to_owned()) };
    let mut fields_parsed = Vec::new();
    for field in fields {
        let ValueKind::Tuple(mut pair) = field.node else { return Err("a field".to_owned()) };
        let _ = pair.pop();
        let field_name = pair.pop().ok_or("a field has a name")?;
        let byte = bytes.pop_front().ok_or("packet too short")?;
        fields_parsed.push(v::tuple(vec![field_name, p4.case("BitV %", vec![v::nat(byte)])?]));
    }
    let fields =
        Value { node: ValueKind::List(fields_parsed), note: fields_note, span: Default::default() };
    let hdr = p4.case("HdrV % %", vec![name, fields])?;
    let pkt = v::external(typ_var("pkt"), serde_json::json!(bytes));
    Ok((pkt, hdr))
}

/// Calls `P4Block_call` with its inputs and returns its outputs, through some tower.
pub type BlockCall<'a> =
    dyn FnMut(&mut Arena, Vec<flat::Value>) -> Result<Vec<flat::Value>, Error> + 'a;

/// Pass or drop, decided by the filter block.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Drop,
}

/// The fixed-function blocks around a P4 program.
pub struct Simulator<'reg> {
    p4: &'reg Registry,
}

impl<'reg> Simulator<'reg> {
    pub fn new(p4: &'reg Registry) -> Self {
        Self { p4 }
    }

    /// Runs `prs` and then `flt` on `bytes`, calling `P4Block_call` through `call`.
    ///
    /// `call` receives the inputs of `P4Block_call` (program, block name, arguments)
    /// and returns its outputs, the final values of the block's parameters.
    pub fn run(
        &self,
        arena: &mut Arena,
        program: flat::Value,
        bytes: &[u8],
        call: &mut BlockCall<'_>,
    ) -> Result<Verdict, Error> {
        let p4 = self.p4;
        // Initiate the parser on the packet and an empty header of type H
        let pkt = p4.case("PktV %", vec![v::external(typ_var("pkt"), serde_json::json!(bytes))])?;
        let zero = || p4.case("BitV %", vec![v::nat(0)]);
        let field =
            |name: &str| -> Result<Value, Error> { Ok(v::tuple(vec![v::text(name), zero()?])) };
        let fields = v::list(typ_var("E"), vec![field("src")?, field("dst")?]);
        let hdr = p4.case("HdrV % %", vec![v::text("H"), fields])?;
        let args = vec![v::intern(arena, pkt)?, v::intern(arena, hdr)?];
        let outs = self.block(arena, program, "prs", args, call)?;
        let [_, hdr] = <[_; 2]>::try_from(outs).map_err(|_| "prs has two parameters")?;
        // Run the filter on the parsed header; pass is an out parameter
        let pass = v::intern(arena, p4.case("BoolV %", vec![v::bool(false)])?)?;
        let outs = self.block(arena, program, "flt", vec![hdr, pass], call)?;
        let [_, pass] = <[_; 2]>::try_from(outs).map_err(|_| "flt has two parameters")?;
        let (ctor, mut args) = v::uncase(v::read(arena, pass))?;
        match (ctor.as_str(), args.pop().map(|arg| arg.node)) {
            ("BoolV %", Some(ValueKind::Bool(true))) => Ok(Verdict::Pass),
            ("BoolV %", Some(ValueKind::Bool(false))) => Ok(Verdict::Drop),
            _ => Err("flt sets pass to a boolean".to_owned()),
        }
    }

    fn block(
        &self,
        arena: &mut Arena,
        program: flat::Value,
        name: &str,
        args: Vec<flat::Value>,
        call: &mut BlockCall<'_>,
    ) -> Result<Vec<flat::Value>, Error> {
        let name = v::intern(arena, v::text(name))?;
        let args = v::read_many(arena, &args);
        let args = v::intern(arena, v::list(typ_var("p4Val"), args))?;
        let outs = call(arena, vec![program, name, args])?;
        let [outs] = <[_; 1]>::try_from(outs).map_err(|_| "P4Block_call has one output")?;
        match v::read(arena, outs).node {
            ValueKind::List(outs) => outs.into_iter().map(|out| v::intern(arena, out)).collect(),
            _ => Err("P4Block_call outputs a list".to_owned()),
        }
    }
}
