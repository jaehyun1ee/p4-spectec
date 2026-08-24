use std::{path::Path, process::Command};

use p4spec_rust::wire::{
    AL_SCHEMA, EL_SCHEMA, Envelope, IL_SCHEMA, PL_SCHEMA, SL_SCHEMA,
    ocaml::lang::{al, el, il, pl, sl},
};
use serde_json::Value;

#[derive(Clone, Copy)]
enum Stage {
    El,
    Il,
    Al,
    Sl,
    Pl,
}

impl Stage {
    const ALL: [Self; 5] = [Self::El, Self::Il, Self::Al, Self::Sl, Self::Pl];

    fn name(self) -> &'static str {
        match self {
            Self::El => "el",
            Self::Il => "il",
            Self::Al => "al",
            Self::Sl => "sl",
            Self::Pl => "pl",
        }
    }

    fn schema(self) -> &'static str {
        match self {
            Self::El => EL_SCHEMA,
            Self::Il => IL_SCHEMA,
            Self::Al => AL_SCHEMA,
            Self::Sl => SL_SCHEMA,
            Self::Pl => PL_SCHEMA,
        }
    }
}

fn export_stage(repo: &Path, stage: Stage) -> Vec<u8> {
    let output = Command::new("opam")
        .args([
            "exec",
            "--",
            "dune",
            "exec",
            "--root",
            repo.to_str().expect("UTF-8 repository path"),
            "./p4spec/bin/main.exe",
            "--",
            "export-json",
            "-stage",
            stage.name(),
            "spec",
        ])
        .current_dir(repo)
        .output()
        .expect("run pinned OCaml exporter");
    assert!(
        output.status.success(),
        "{} export failed:\n{}",
        stage.name(),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn round_trip(stage: Stage, payload: &Value) -> Value {
    match stage {
        Stage::El => el::SpecCodec::encode(&el::SpecCodec::decode(payload).expect("decode EL"))
            .expect("encode EL"),
        Stage::Il => il::SpecCodec::encode(&il::SpecCodec::decode(payload).expect("decode IL"))
            .expect("encode IL"),
        Stage::Al => al::SpecCodec::encode(&al::SpecCodec::decode(payload).expect("decode AL"))
            .expect("encode AL"),
        Stage::Sl => sl::SpecCodec::encode(&sl::SpecCodec::decode(payload).expect("decode SL"))
            .expect("encode SL"),
        Stage::Pl => pl::SpecCodec::encode(&pl::SpecCodec::decode(payload).expect("decode PL"))
            .expect("encode PL"),
    }
}

fn normalize_generated_metadata(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(normalize_generated_metadata),
        Value::Object(fields) => {
            if fields.contains_key("vid") {
                fields.insert("vid".to_owned(), Value::from(0));
            }
            if fields.contains_key("vhash") {
                fields.insert("vhash".to_owned(), Value::from(0));
            }
            fields.values_mut().for_each(normalize_generated_metadata);
        }
        _ => {}
    }
}

#[test]
fn ocaml_full_corpus_roundtrips_all_stage_asts() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Rust crate is inside the repository");

    for stage in Stage::ALL {
        let document = export_stage(repo, stage);
        let envelope = Envelope::<Value>::from_slice(&document).expect("decode stage envelope");
        assert_eq!(envelope.schema(), stage.schema());
        assert_eq!(envelope.kind(), stage.name());

        let mut expected = envelope.into_payload();
        let mut actual = round_trip(stage, &expected);
        normalize_generated_metadata(&mut expected);
        normalize_generated_metadata(&mut actual);
        assert_eq!(actual, expected, "{} stage changed", stage.name());
    }
}
