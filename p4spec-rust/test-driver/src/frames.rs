//! Streams independent semantic frames against the cached source simulator

use std::{
    collections::HashSet,
    io::{BufRead, BufReader, BufWriter, Write},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use p4spec_rust::{
    lang::{
        common::{
            Iter,
            notation::{atom::Atom, mixfix::Mixfix},
        },
        data::{
            typ::TypKind,
            value::{Value, ValueArena, ValueKind, serde},
        },
        xl::num::{self, Number},
    },
    sim_plugin::{io::Transmission, runner::Run},
};
use serde_json::{Value as Json, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("source simulation protocol: {0}")]
    Protocol(String),
    #[error("simulation frame {frame} in {case}: native {native}; source {source_frame}")]
    Difference {
        case: String,
        frame: u64,
        native: String,
        source_frame: String,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub frames: u64,
    pub commands: usize,
    pub states: usize,
}

pub struct Worker {
    child: Child,
    input: BufWriter<ChildStdin>,
    output: BufReader<ChildStdout>,
    line: String,
    arch: String,
    cases: HashSet<String>,
    case: Option<String>,
    counts: Counts,
}

impl Worker {
    pub fn spawn(
        path_executable: &Path,
        path_spec: &Path,
        arch: &str,
        det: bool,
    ) -> Result<Self, Error> {
        let mut child = Command::new(path_executable)
            .arg(path_spec)
            .arg(arch)
            .arg(det.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = BufWriter::new(
            child
                .stdin
                .take()
                .ok_or_else(|| Error::Protocol("missing stdin".into()))?,
        );
        let output = BufReader::with_capacity(
            256 * 1024,
            child
                .stdout
                .take()
                .ok_or_else(|| Error::Protocol("missing stdout".into()))?,
        );
        let mut worker = Self {
            child,
            input,
            output,
            line: String::new(),
            arch: arch.into(),
            cases: HashSet::new(),
            case: None,
            counts: Counts::default(),
        };
        worker.compare(json!(["Ready", arch, det]))?;
        Ok(worker)
    }

    pub fn begin_case(
        &mut self,
        id: &str,
        path_p4: &Path,
        path_stf: &Path,
        includes: &[std::path::PathBuf],
    ) -> Result<(), Error> {
        if self.case.is_some() || !self.cases.insert(id.into()) {
            return Err(Error::Protocol(format!(
                "duplicate or overlapping case {id}"
            )));
        }
        self.case = Some(id.into());
        self.counts = Counts::default();
        serde_json::to_writer(
            &mut self.input,
            &json!({"id": id, "p4": path_p4, "stf": path_stf, "includes": includes}),
        )?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        self.compare(json!(["Case", id]))
    }

    pub fn compare_state(
        &mut self,
        boundary: &str,
        command: usize,
        arena: &ValueArena,
        run: &Run,
        txs: &[Transmission],
    ) -> Result<(), Error> {
        let Run {
            state,
            tx_output_queue: outputs,
            expect_queue: expects,
            ..
        } = run;
        self.compare(json!(["State", boundary, command]))?;
        self.value(arena, &state.value_ctx)?;
        self.value(arena, &state.value_arch)?;
        self.compare(json!(["Transmissions", txs.len()]))?;
        for tx in txs {
            self.tx(tx)?;
        }
        self.compare(json!(["Outputs", outputs.len()]))?;
        for tx in outputs {
            self.tx(tx)?;
        }
        self.compare(json!(["Expects", expects.len()]))?;
        for expect in expects {
            self.tx(&expect.tx)?;
            self.compare(json!(["Exact", expect.exact]))?;
        }
        self.compare(json!(["StateEnd"]))?;
        self.counts.states += 1;
        self.counts.commands = command;
        Ok(())
    }

    pub fn end_case(&mut self, status: &str) -> Result<Counts, Error> {
        if self.case.is_none() {
            return Err(Error::Protocol("no active case".into()));
        }
        self.compare(json!([
            "CaseEnd",
            status,
            self.counts.commands,
            self.counts.states
        ]))?;
        self.case = None;
        Ok(self.counts)
    }

    pub fn finish(&mut self) -> Result<(), Error> {
        if self.case.is_some() {
            return Err(Error::Protocol("unfinished case".into()));
        }
        self.input.write_all(b"{\"quit\":true}\n")?;
        self.input.flush()?;
        self.compare(json!(["Done", self.cases.len()]))?;
        self.line.clear();
        if self.output.read_line(&mut self.line)? != 0 {
            return Err(Error::Protocol(format!(
                "unexpected trailing source output: {}",
                bounded(&self.line)
            )));
        }
        let status = self.child.wait()?;
        if !status.success() {
            return Err(Error::Protocol(format!("source worker exited {status}")));
        }
        Ok(())
    }

    fn read(&mut self) -> Result<Json, Error> {
        loop {
            self.line.clear();
            if self.output.read_line(&mut self.line)? == 0 {
                return Err(Error::Protocol(format!(
                    "premature EOF in {:?}: {:?}",
                    self.case,
                    self.child.try_wait()?
                )));
            }
            if let Some(text) = self.line.strip_prefix("SIM-FRAME ") {
                return Ok(serde_json::from_str(text)?);
            }
            // The source frontend can print progress before the protocol begins
            if self.case.is_some() {
                return Err(Error::Protocol(format!(
                    "unexpected source output: {}",
                    bounded(&self.line)
                )));
            }
        }
    }

    fn compare(&mut self, native: Json) -> Result<(), Error> {
        let source_frame = self.read()?;
        self.counts.frames += 1;
        if native != source_frame {
            return Err(Error::Difference {
                case: self.case.clone().unwrap_or_else(|| "startup".into()),
                frame: self.counts.frames,
                native: bounded(&native.to_string()),
                source_frame: bounded(&source_frame.to_string()),
            });
        }
        Ok(())
    }

    fn tx(&mut self, tx: &Transmission) -> Result<(), Error> {
        self.compare(json!(["Tx", tx.port, tx.packet]))
    }

    fn value(&mut self, arena: &ValueArena, value: &Value) -> Result<(), Error> {
        let typ = semantic_typ(arena.typ(value));
        match arena.kind(value) {
            ValueKind::Bool(value) => self.compare(json!(["Value", "Bool", typ, value])),
            ValueKind::Num(Number::Nat(value)) => {
                self.compare(json!(["Value", "Nat", typ, value.to_string()]))
            }
            ValueKind::Num(Number::Int(value)) => {
                self.compare(json!(["Value", "Int", typ, value.to_string()]))
            }
            ValueKind::Text(value) => self.compare(json!(["Value", "Text", typ, value])),
            ValueKind::Func(id) => self.compare(json!(["Value", "Func", typ, id.node])),
            ValueKind::Struct(fields) => {
                self.compare(json!(["Value", "Struct", typ, fields.len()]))?;
                for (atom, value) in fields {
                    self.compare(json!(["Field", atom_frame(&atom.node)]))?;
                    self.value(arena, value)?;
                }
                Ok(())
            }
            ValueKind::Tuple(values) | ValueKind::List(values) => {
                let name = if matches!(arena.kind(value), ValueKind::Tuple(_)) {
                    "Tuple"
                } else {
                    "List"
                };
                self.compare(json!(["Value", name, typ, values.len()]))?;
                for value in values {
                    self.value(arena, value)?;
                }
                Ok(())
            }
            ValueKind::Opt(value) => {
                self.compare(json!(["Value", "Opt", typ, value.is_some()]))?;
                if let Some(value) = value {
                    self.value(arena, value)?;
                }
                Ok(())
            }
            ValueKind::Case(mixfix) => {
                self.compare(json!(["Value", "Case", typ]))?;
                self.mixfix(arena, mixfix)
            }
            ValueKind::Extern(json) => {
                self.compare(json!(["Value", "Extern", typ]))?;
                let route = match arena.typ(value).as_ref() {
                    TypKind::Var(id, targs)
                        if targs.is_empty() && id.node == "archState" && self.arch != "ebpf" =>
                    {
                        "arch"
                    }
                    TypKind::Var(id, targs) if targs.is_empty() && id.node == "objectState" => {
                        "object"
                    }
                    _ => "raw",
                };
                self.external(route, json)
            }
        }
    }

    fn mixfix(&mut self, arena: &ValueArena, mixfix: &Mixfix<Value>) -> Result<(), Error> {
        match mixfix {
            Mixfix::Arg(value) => {
                self.compare(json!(["Mixfix", "Arg"]))?;
                self.value(arena, value)
            }
            Mixfix::Atom(atom) => self.compare(json!(["Mixfix", "Atom", atom_frame(&atom.node)])),
            Mixfix::Brack(atom_l, mixfix, atom_r) => {
                self.compare(json!([
                    "Mixfix",
                    "Brack",
                    atom_frame(&atom_l.node),
                    atom_frame(&atom_r.node)
                ]))?;
                self.mixfix(arena, mixfix)
            }
            Mixfix::Infix(mixfix_l, atom, mixfix_r) => {
                self.compare(json!(["Mixfix", "Infix", atom_frame(&atom.node)]))?;
                self.mixfix(arena, mixfix_l)?;
                self.mixfix(arena, mixfix_r)
            }
            Mixfix::Seq(mixfixes) => {
                self.compare(json!(["Mixfix", "Seq", mixfixes.len()]))?;
                for mixfix in mixfixes {
                    self.mixfix(arena, mixfix)?;
                }
                Ok(())
            }
        }
    }

    // Only typed arch/object paths decode nested values or reconcile serde shapes
    fn external(&mut self, route: &str, data: &Json) -> Result<(), Error> {
        match route {
            "value" => {
                let mut arena = ValueArena::new();
                let value: Value = serde::decode(&mut arena, data)?;
                return self.value(&arena, &value);
            }
            "object" if data.is_object() => {
                let fields = data.as_object().expect("object checked");
                if fields.len() != 1 {
                    return self.external("raw", data);
                }
                let (name, data_payload) = fields.iter().next().expect("one variant");
                let route = match name.as_str() {
                    "PacketIn" | "PacketOut" => "record",
                    "Register" => "register",
                    "Counter" => "counter",
                    "DirectCounter" | "DirectMeter" => "direct_counter",
                    "Meter" => "meter",
                    "InternetChecksum" => "checksum",
                    "CounterArray" => "counter_array",
                    "Hash" => "hash",
                    _ => return self.external("raw", data),
                };
                self.compare(json!(["External", "List", 2]))?;
                self.external("raw", &json!(name))?;
                return self.external(route, data_payload);
            }
            "checksum" | "counter_array" | "hash" => {
                let name = match route {
                    "checksum" => "int",
                    "counter_array" => "counts",
                    _ => "algo",
                };
                let fields = data
                    .as_object()
                    .ok_or_else(|| Error::Protocol("invalid native object wrapper".into()))?;
                if fields.len() != 1 {
                    return Err(Error::Protocol("invalid native object wrapper".into()));
                }
                let data = fields.get(name).ok_or_else(|| {
                    Error::Protocol(format!("missing native object field {name}"))
                })?;
                return self.external(if route == "checksum" { "bigint" } else { "raw" }, data);
            }
            "counter" | "direct_counter" | "meter" => {
                let fields = data.as_object().ok_or_else(|| {
                    Error::Protocol("invalid native counter/meter variant".into())
                })?;
                if fields.len() != 1 {
                    return Err(Error::Protocol(
                        "invalid native counter/meter variant".into(),
                    ));
                }
                let (name, data) = fields.iter().next().expect("one variant");
                self.compare(json!(["External", "List", 2]))?;
                self.external("raw", &json!(name))?;
                return self.external(
                    if route == "meter" {
                        "colors"
                    } else if route == "counter" {
                        if name == "PacketsAndBytes" {
                            "bigint_pairs"
                        } else {
                            "bigints"
                        }
                    } else if name == "PacketsAndBytes" {
                        "bigints"
                    } else {
                        "bigint"
                    },
                    data,
                );
            }
            "bigint" => {
                let int: num_bigint::BigInt = serde_json::from_value(data.clone())?;
                return self.external("raw", &json!(int.to_string()));
            }
            "entrypoint" | "color" => {
                let name = data
                    .as_str()
                    .ok_or_else(|| Error::Protocol("invalid native unit variant".into()))?;
                let name = if route == "color" {
                    match name {
                        "Red" => "RED",
                        "Green" => "GREEN",
                        "Yellow" => "YELLOW",
                        _ => return Err(Error::Protocol("unknown meter color".into())),
                    }
                } else {
                    name
                };
                return self.external("raw", &json!([name]));
            }
            "clone" if !data.is_null() => {
                let values = data
                    .as_array()
                    .ok_or_else(|| Error::Protocol("invalid clone tuple".into()))?;
                if values.len() != 3 {
                    return Err(Error::Protocol("invalid clone tuple".into()));
                }
                self.compare(json!(["External", "List", 3]))?;
                self.external("entrypoint", &values[0])?;
                self.external("raw", &values[1])?;
                return self.external("raw", &values[2]);
            }
            _ => {}
        }
        match data {
            Json::Null => self.compare(json!(["External", "Null"])),
            Json::Bool(value) => self.compare(json!(["External", "Bool", value])),
            Json::Number(num) => {
                if num.is_f64() {
                    self.compare(json!([
                        "External",
                        "Float",
                        (num.as_f64().expect("float").to_bits() as i64).to_string()
                    ]))
                } else {
                    self.compare(json!(["External", "Int", num]))
                }
            }
            Json::String(text) => self.compare(json!(["External", "String", text])),
            Json::Array(values) => {
                let route = match route {
                    "queue" => "packet",
                    "values" => "value",
                    "records" => "record",
                    "bigints" => "bigint",
                    "bigint_pairs" => "bigints",
                    "colors" => "color",
                    _ => "raw",
                };
                self.compare(json!(["External", "List", values.len()]))?;
                for data in values {
                    self.external(route, data)?;
                }
                Ok(())
            }
            Json::Object(fields) => {
                let mut fields: Vec<_> = fields
                    .iter()
                    .map(|(name, data)| (name.as_str(), data))
                    .collect();
                if matches!(route, "map" | "groups" | "nodes") {
                    let mut fields_numbered = Vec::with_capacity(fields.len());
                    for (name, data) in fields {
                        let num = name
                            .parse::<i64>()
                            .map_err(|_| Error::Protocol("invalid numeric map key".into()))?;
                        fields_numbered.push((num, name, data));
                    }
                    fields_numbered.sort_by_key(|(num, _, _)| *num);
                    fields = fields_numbered
                        .into_iter()
                        .map(|(_, name, data)| (name, data))
                        .collect();
                } else if route != "raw" {
                    fields.sort_by_key(|(name, _)| match (route, *name) {
                        ("multicast", "handle_next") => "next_handle",
                        ("register", "value_typ") => "typ",
                        _ => *name,
                    });
                }
                self.compare(json!(["External", "Assoc", fields.len()]))?;
                for (name, data) in fields {
                    let name_source = match (route, name) {
                        ("multicast", "handle_next") => "next_handle",
                        ("register", "value_typ") => "typ",
                        _ => name,
                    };
                    self.compare(json!(["ExternalField", name_source]))?;
                    let route_child = match (route, name) {
                        ("arch", "queue") => "queue",
                        ("arch", "mirrortable") => "map",
                        ("arch", "multicast") => "multicast",
                        ("arch", "action") => "action",
                        ("packet", "value_ctx") => "value",
                        ("packet", "packet_in") => "record",
                        ("packet", "entrypoint") => "entrypoint",
                        ("multicast", "groups") => "groups",
                        ("multicast", "nodes") => "nodes",
                        ("nodes", _) => "records",
                        ("register", "value_typ") => "value",
                        ("register", "values") => "values",
                        ("action", "clone_opt") => "clone",
                        _ => "raw",
                    };
                    if route == "groups" && self.arch == "v1model" {
                        let id = name
                            .parse::<i64>()
                            .map_err(|_| Error::Protocol("invalid group id".into()))?;
                        self.external("record", &json!({"id": id, "node_handles": data}))?;
                    } else {
                        self.external(route_child, data)?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn bounded(text: &str) -> String {
    text.chars().take(512).collect()
}

fn semantic_typ(typ: &TypKind) -> Json {
    match typ {
        TypKind::Bool => json!(["BoolT"]),
        TypKind::Num(num::Typ::Nat) => json!(["NumT", "NatT"]),
        TypKind::Num(num::Typ::Int) => json!(["NumT", "IntT"]),
        TypKind::Text => json!(["TextT"]),
        TypKind::Var(id, targs) => json!([
            "VarT",
            id.node,
            targs
                .iter()
                .map(|typ| semantic_typ(&typ.node))
                .collect::<Vec<_>>()
        ]),
        TypKind::Tuple(typs) => json!([
            "TupleT",
            typs.iter()
                .map(|typ| semantic_typ(&typ.node))
                .collect::<Vec<_>>()
        ]),
        TypKind::Iter(typ, iter) => json!([
            "IterT",
            semantic_typ(&typ.node),
            match iter {
                Iter::Opt => "Opt",
                Iter::List => "List",
            }
        ]),
        TypKind::Func(typ) => json!([
            "FuncT",
            typ.tparams.iter().map(|id| &id.node).collect::<Vec<_>>(),
            typ.typs_params
                .iter()
                .map(|typ| semantic_typ(&typ.node))
                .collect::<Vec<_>>(),
            semantic_typ(&typ.typ_ret.node)
        ]),
    }
}

fn atom_frame(atom: &Atom) -> Json {
    match atom {
        Atom::Keyword(text) => json!(["Keyword", text]),
        Atom::Tag(text) => json!(["Tag", text]),
        Atom::Operator(text) => json!(["Operator", text]),
        Atom::Sub => json!(["Sub"]),
        Atom::Sup => json!(["Sup"]),
        Atom::Turnstile => json!(["Turnstile"]),
        Atom::Tilesturn => json!(["Tilesturn"]),
        Atom::Arrow => json!(["Arrow"]),
        Atom::ArrowSub => json!(["ArrowSub"]),
        Atom::DoubleArrowSub => json!(["DoubleArrowSub"]),
        Atom::DoubleArrowLong => json!(["DoubleArrowLong"]),
        Atom::SqArrow => json!(["SqArrow"]),
        Atom::SqArrowStar => json!(["SqArrowStar"]),
        Atom::Dot => json!(["Dot"]),
        Atom::Dot2 => json!(["Dot2"]),
        Atom::Dot3 => json!(["Dot3"]),
        Atom::Semicolon => json!(["Semicolon"]),
        Atom::Colon => json!(["Colon"]),
        Atom::ColonEq => json!(["ColonEq"]),
        Atom::Tilde2 => json!(["Tilde2"]),
        Atom::Backslash => json!(["Backslash"]),
        Atom::LAngle => json!(["LAngle"]),
        Atom::RAngle => json!(["RAngle"]),
        Atom::LParen => json!(["LParen"]),
        Atom::RParen => json!(["RParen"]),
        Atom::LBrack => json!(["LBrack"]),
        Atom::RBrack => json!(["RBrack"]),
        Atom::LBrace => json!(["LBrace"]),
        Atom::RBrace => json!(["RBrace"]),
    }
}
