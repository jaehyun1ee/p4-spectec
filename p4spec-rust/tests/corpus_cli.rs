use std::{fs, process::Command};

use p4spec_rust::{
    domain::{
        mixfix::Mixfix,
        source::{Region, Spanned},
    },
    lang::{il::ast as il, sl::ast as sl},
    wire::{
        Envelope,
        ocaml::lang::{il::ValueEnvelopeCodec, sl::SpecCodec},
    },
};

fn span(file: &str) -> Region {
    Region::for_file(file)
}

fn id(name: &str) -> il::Id {
    Spanned::new(name.to_owned(), span(name))
}

fn signature() -> sl::RelSignature {
    (
        Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
        Vec::new(),
    )
}

fn variable(name: &str) -> il::Exp {
    il::Exp::new(il::ExpKind::VarE(id(name)), il::TypKind::TextT, span(name))
}

fn program_inst_spec() -> sl::Spec {
    let item = variable("item");
    let tuple = il::Exp::new(
        il::ExpKind::TupleE(vec![item.clone()]),
        il::TypKind::TupleT(vec![Spanned::new(il::TypKind::TextT, span("text"))]),
        span("tuple"),
    );
    vec![Spanned::new(
        sl::DefKind::RelD((
            id("Program_inst"),
            signature(),
            vec![tuple],
            vec![sl::Instr::new(
                sl::InstrKind::ResultI(signature(), vec![item]),
                1,
                span("result"),
            )],
            None,
            Vec::new(),
        )),
        span("Program_inst"),
    )]
}

fn write_value(path: &std::path::Path, value: &str) {
    let value = il::Value::new(
        il::ValueKind::TextV(value.to_owned()),
        il::TypKind::TextT,
        span(value),
    );
    fs::write(path, ValueEnvelopeCodec::encode(&value).unwrap()).unwrap();
}

fn write_tuple(path: &std::path::Path, value: &str) {
    let text_type = Spanned::new(il::TypKind::TextT, span("text"));
    let value = il::Value::new(
        il::ValueKind::TupleV(vec![il::Value::new(
            il::ValueKind::TextV(value.to_owned()),
            il::TypKind::TextT,
            span(value),
        )]),
        il::TypKind::TupleT(vec![text_type]),
        span("tuple"),
    );
    fs::write(path, ValueEnvelopeCodec::encode(&value).unwrap()).unwrap();
}

#[test]
fn corpus_driver_reports_every_program_and_times_only_program_inst() {
    let directory = std::env::temp_dir();
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("corpus-cli")
    );
    let spec_path = directory.join(format!("p4spec-rust-{suffix}-spec.json"));
    let rejected_path = directory.join(format!("p4spec-rust-{suffix}-rejected.json"));
    let accepted_path = directory.join(format!("p4spec-rust-{suffix}-accepted.json"));
    let records_path = directory.join(format!("p4spec-rust-{suffix}-records.jsonl"));
    let spec_payload = SpecCodec::encode(&program_inst_spec()).unwrap();
    fs::write(
        &spec_path,
        serde_json::to_vec(&Envelope::sl(spec_payload)).unwrap(),
    )
    .unwrap();
    write_value(&rejected_path, "rejected");
    write_tuple(&accepted_path, "accepted");

    let binary = std::env::var("CARGO_BIN_EXE_p4spec-rust-corpus").unwrap();
    let output = Command::new(&binary)
        .args([
            "--spec",
            spec_path.to_str().unwrap(),
            "--expect",
            "any",
            "--warmup",
            "1",
            "--repeat",
            "2",
            "--output",
            records_path.to_str().unwrap(),
            rejected_path.to_str().unwrap(),
            accepted_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let records = fs::read_to_string(&records_path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 4);
    assert_eq!(records[0]["program"], rejected_path.to_str().unwrap());
    assert_eq!(records[0]["iteration"], 1);
    assert_eq!(records[0]["relation"], "Program_inst");
    assert_eq!(records[0]["status"], "fail");
    assert!(records[0]["decode_ns"].is_u64());
    assert!(records[0]["eval_ns"].is_u64());
    assert!(records[0]["output_count"].is_null());
    assert_eq!(records[1]["program"], rejected_path.to_str().unwrap());
    assert_eq!(records[1]["iteration"], 2);
    assert_eq!(records[2]["program"], accepted_path.to_str().unwrap());
    assert_eq!(records[2]["iteration"], 1);
    assert_eq!(records[2]["relation"], "Program_inst");
    assert_eq!(records[2]["status"], "pass");
    assert!(records[2]["decode_ns"].is_u64());
    assert!(records[2]["eval_ns"].is_u64());
    assert_eq!(records[2]["output_count"], 1);
    assert_eq!(records[3]["program"], accepted_path.to_str().unwrap());
    assert_eq!(records[3]["iteration"], 2);
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("evaluations=4")
    );

    let mismatch = Command::new(binary)
        .args([
            "--spec",
            spec_path.to_str().unwrap(),
            rejected_path.to_str().unwrap(),
            accepted_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    fs::remove_file(spec_path).unwrap();
    fs::remove_file(rejected_path).unwrap();
    fs::remove_file(accepted_path).unwrap();
    fs::remove_file(records_path).unwrap();
    assert!(!mismatch.status.success());
    assert_eq!(
        String::from_utf8(mismatch.stdout).unwrap().lines().count(),
        2
    );
}
