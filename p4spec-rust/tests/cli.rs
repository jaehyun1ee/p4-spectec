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

fn identity_spec() -> sl::Spec {
    let value = il::Exp::new(
        il::ExpKind::VarE(id("value")),
        il::TypKind::TextT,
        span("value"),
    );
    let signature = || {
        (
            Spanned::new(Mixfix::Seq(Vec::new()), span("signature")),
            Vec::new(),
        )
    };
    vec![Spanned::new(
        sl::DefKind::RelD((
            id("identity"),
            signature(),
            vec![value.clone()],
            vec![sl::Instr::new(
                sl::InstrKind::ResultI(signature(), vec![value]),
                1,
                span("result"),
            )],
            None,
            Vec::new(),
        )),
        span("identity"),
    )]
}

#[test]
fn run_command_decodes_ocaml_envelopes_and_emits_value_envelopes() {
    let directory = std::env::temp_dir();
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("cli")
    );
    let spec_path = directory.join(format!("p4spec-rust-{suffix}-spec.json"));
    let value_path = directory.join(format!("p4spec-rust-{suffix}-value.json"));
    let spec_payload = SpecCodec::encode(&identity_spec()).unwrap();
    fs::write(
        &spec_path,
        serde_json::to_vec(&Envelope::sl(spec_payload)).unwrap(),
    )
    .unwrap();
    let value = il::Value::new(
        il::ValueKind::TextV("program".to_owned()),
        il::TypKind::TextT,
        span("program"),
    );
    fs::write(&value_path, ValueEnvelopeCodec::encode(&value).unwrap()).unwrap();

    let output = Command::new(std::env::var("CARGO_BIN_EXE_p4spec-rust").unwrap())
        .args([
            "run",
            "--spec",
            spec_path.to_str().unwrap(),
            "--relation",
            "identity",
            value_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    fs::remove_file(spec_path).unwrap();
    fs::remove_file(value_path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decoded = ValueEnvelopeCodec::decode(&output.stdout).unwrap();
    assert_eq!(decoded, value);
}
