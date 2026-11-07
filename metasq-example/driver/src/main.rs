//! Mini P4 and p4st-in-p4st specifications, run on the p4spectec library
//!
//! `meta` runs filter.p4 on the mini P4 specification,
//! p4 ◁ p4st ◀ rust, for a few packets.
//! `metameta` runs the same packets through the mini p4st-in-p4st
//! specification, p4 ◁ p4st ◁ p4st ◀ rust, and checks that
//! both towers agree.
//! `emit` prints the encoded specification and the walk-through call
//! for the mini K and Redex specifications.

mod emit;
mod encode;
mod engine;
mod externs;
mod value;

use std::{path::PathBuf, process::ExitCode, rc::Rc};

use p4spectec::{
    lang::{al::ast as al, data::arena::Arena, data::value::flat, data::value::tree::ValueKind},
    runner::{self, Config},
};

use crate::{
    encode::Encoder,
    engine::{BlockCall, Simulator, Verdict},
    externs::{PacketEngine, Relay},
    value::{self as v, Error, Registry, typ_var},
};

/// Packets for filter.p4, as (src, dst) bytes, with the verdict filter.p4 implies.
const PACKETS: [([u8; 2], Verdict); 3] =
    [([0x01, 0x42], Verdict::Drop), ([0x01, 0x43], Verdict::Pass), ([0x42, 0x00], Verdict::Pass)];

/// The metasq-example directory, holding the specifications.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The specification files of a directory, in name order.
pub fn files(dir: &str) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(root().join(dir))
        .expect("specification directory exists")
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "watsup"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn algo(paths: &[PathBuf]) -> Result<al::Spec, Error> {
    p4spectec::algo(paths).map_err(|report| report.to_string())
}

/// The mini P4 specification, without and with the program filter.p4.
fn p4_specs() -> Result<(al::Spec, al::Spec), Error> {
    let spec = algo(&[root().join("p4/p4.watsup")])?;
    let spec_with_program = algo(&[root().join("p4/p4.watsup"), root().join("p4/filter.watsup")])?;
    Ok((spec, spec_with_program))
}

pub fn config() -> Config {
    Config::new(true, true, false)
}

/// Runs every packet through `call` and reports each verdict against the expected one.
fn run_packets(
    arena: &mut Arena,
    p4: &Registry,
    program: flat::Value,
    tower: &str,
    call: &mut BlockCall<'_>,
) -> Result<Vec<Verdict>, Error> {
    let simulator = Simulator::new(p4);
    let mut verdicts = Vec::new();
    for (bytes, expected) in PACKETS {
        let verdict = simulator.run(arena, program, &bytes, call)?;
        let mark = if verdict == expected { "ok" } else { "UNEXPECTED" };
        println!("[{tower}] pkt {bytes:02x?} -> {verdict:?} ({mark})");
        verdicts.push(verdict);
    }
    Ok(verdicts)
}

/// Meta-execution: the mini P4 specification on the native AL interpreter.
fn meta(arena: &mut Arena) -> Result<Vec<Verdict>, Error> {
    let (_, spec) = p4_specs()?;
    let p4 = Rc::new(Registry::new(&spec));
    let external = PacketEngine { p4: p4.clone() };
    let mut runner =
        runner::build_al(arena, spec, config(), external).map_err(|r| r.to_string())?;
    let program = runner
        .eval_func(arena, "filter_p4", &[], &[])
        .result
        .map_err(|e| e.to_string())?;
    run_packets(arena, &p4, program, "p4 ◁ p4st ◀ rust", &mut |arena, inputs| {
        runner
            .eval_rel(arena, "P4Block_call", &inputs)
            .result
            .map_err(|e| e.to_string())
    })
}

/// Meta-meta-execution: the mini P4 specification run by mini p4st-in-p4st.
fn metameta(arena: &mut Arena) -> Result<Vec<Verdict>, Error> {
    let (spec_p4, spec_with_program) = p4_specs()?;
    let p4 = Rc::new(Registry::new(&spec_with_program));
    // The program comes from the frontend, here the native run of $filter_p4
    let external = PacketEngine { p4: p4.clone() };
    let mut frontend = runner::build_al(arena, spec_with_program, config(), external)
        .map_err(|r| r.to_string())?;
    let program = frontend
        .eval_func(arena, "filter_p4", &[], &[])
        .result
        .map_err(|e| e.to_string())?;
    // Encode the mini P4 specification once, as the input of RunRel
    let spec_st = algo(&files("st-in-st"))?;
    let st = Rc::new(Registry::new(&spec_st));
    let spec_hat = Encoder::new(&st).spec(&spec_p4)?;
    let spec_hat = v::intern(arena, spec_hat)?;
    let id = v::intern(arena, v::text("P4Block_call"))?;
    let external = Relay { p4: p4.clone(), st: st.clone() };
    let mut runner =
        runner::build_al(arena, spec_st, config(), external).map_err(|r| r.to_string())?;
    run_packets(arena, &p4, program, "p4 ◁ p4st ◁ p4st ◀ rust", &mut |arena, inputs| {
        // Encode the call, run RunRel, and decode its result
        let encoder = Encoder::new(&st);
        let vals = v::read_many(arena, &inputs)
            .into_iter()
            .map(|input| encoder.val(input))
            .collect::<Result<_, _>>()?;
        let vals = v::intern(arena, v::list(typ_var("val"), vals))?;
        let outputs = runner
            .eval_rel(arena, "RunRel", &[spec_hat, id, vals])
            .result
            .map_err(|e| e.to_string())?;
        let [vs_res] = <[_; 1]>::try_from(outputs).map_err(|_| "RunRel has one output")?;
        let (ctor, mut args) = v::uncase(v::read(arena, vs_res))?;
        let (Some(vals), "OK %") = (args.pop(), ctor.as_str()) else {
            return Err(format!("RunRel answered {ctor}"));
        };
        let ValueKind::List(vals) = vals.node else { return Err("OK holds val*".to_owned()) };
        vals.into_iter()
            .map(|val| Encoder::unval(&p4, val).and_then(|value| v::intern(arena, value)))
            .collect()
    })
}

/// The first line of a rendered failure that says what went wrong.
fn first_line(error: &str) -> String {
    let lines = error.lines().map(str::trim).filter(|line| !line.is_empty());
    lines
        .clone()
        .find(|line| line.starts_with("error"))
        .or(lines.clone().next_back())
        .unwrap_or("")
        .to_owned()
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let mut arena = Arena::new();
    let result = match command.as_str() {
        "meta" => meta(&mut arena).map(|_| ()),
        // Run both towers even when one fails, then compare their outcomes
        "metameta" => {
            let verdicts_meta = meta(&mut arena);
            let verdicts = metameta(&mut arena);
            match (verdicts_meta, verdicts) {
                (Ok(verdicts_meta), Ok(verdicts)) if verdicts_meta == verdicts => {
                    println!("meta-execution and meta-meta-execution agree");
                    Ok(())
                }
                (Ok(_), Ok(_)) => Err("meta-execution and meta-meta-execution disagree".to_owned()),
                (result_meta, result) => Err(format!(
                    "meta-execution: {}\nmeta-meta-execution: {}",
                    result_meta.map_or_else(|e| first_line(&e), |_| "ok".to_owned()),
                    result.map_or_else(|e| first_line(&e), |_| "ok".to_owned()),
                )),
            }
        }
        "emit" => emit::run(std::env::args().skip(2).collect()),
        _ => Err("usage: metasq-example meta | metameta | emit <out-dir>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
