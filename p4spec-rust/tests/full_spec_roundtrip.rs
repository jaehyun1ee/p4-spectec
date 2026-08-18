use std::{env, fs};

use p4spec_rust::wire::{Envelope, ocaml::lang::sl::SpecCodec};
use serde_json::Value;

const FIXTURE_ENV: &str = "P4SPEC_FULL_SL_JSON";

#[test]
#[ignore = "requires P4SPEC_FULL_SL_JSON pointing to an explicit OCaml full-spec export"]
fn full_ocaml_sl_export_round_trips_through_canonical_ast() {
    let path = env::var_os(FIXTURE_ENV)
        .unwrap_or_else(|| panic!("set {FIXTURE_ENV} to the OCaml full-spec export"));
    let bytes = fs::read(&path).expect("read full SL export");
    let envelope = Envelope::<Value>::from_slice(&bytes).expect("decode versioned SL envelope");
    let original = envelope.into_payload();

    let spec = SpecCodec::decode(&original).expect("decode canonical SL spec");
    let encoded = SpecCodec::encode(&spec).expect("encode canonical SL spec");

    assert_eq!(encoded, original);
}
