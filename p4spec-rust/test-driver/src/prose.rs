use crate::{Error, Result, snapshot};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate, prose, structure},
    wire::ocaml::lang::{pl, sl},
};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Instant};

const SOURCE_SL_SHA256: &str = "47cbdceab528370ac99e14bd05c86ce55faa4a3c4f8b0f2c78ceb493e0405e6c";
const SOURCE_PL_SHA256: &str = "4846b7ca7d5d6319fc54d378fdf4d17fda37fa780e17b143ebf5385452d10d33";

fn hash(bytes: &[u8]) -> String {
    format!("{:x}\n", Sha256::digest(bytes))
}

fn decode_payload(path: &Path, schema: &str, kind: &str) -> Result<(Vec<u8>, serde_json::Value)> {
    let bytes = fs::read(path)?;
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    deserializer.disable_recursion_limit();
    let mut envelope = deserializer
        .into_iter::<serde_json::Value>()
        .next()
        .transpose()
        .map_err(|error| Error::Invalid(error.to_string()))?
        .ok_or_else(|| Error::Invalid(format!("{} is empty", path.display())))?;
    let fields = envelope
        .as_object_mut()
        .ok_or_else(|| Error::Invalid(format!("{} is not an object", path.display())))?;
    if fields.get("schema").and_then(serde_json::Value::as_str) != Some(schema)
        || fields.get("kind").and_then(serde_json::Value::as_str) != Some(kind)
    {
        return Err(Error::Invalid(format!(
            "{} is not a {schema} {kind} envelope",
            path.display()
        )));
    }
    let payload = fields
        .remove("payload")
        .ok_or_else(|| Error::Invalid(format!("{} has no payload", path.display())))?;
    Ok((bytes, payload))
}

fn check_source_hash(path: &Path, bytes: &[u8], expected: &str) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(Error::Invalid(format!(
            "{} has source hash {actual}, expected {expected}",
            path.display()
        )));
    }
    Ok(())
}

fn first_difference(
    actual: &serde_json::Value,
    expected: &serde_json::Value,
    path: &mut String,
) -> Option<String> {
    match (actual, expected) {
        (serde_json::Value::Array(actual), serde_json::Value::Array(expected)) => {
            if actual.len() != expected.len() {
                return Some(format!(
                    "{path}: array lengths {} != {}",
                    actual.len(),
                    expected.len()
                ));
            }
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                let path_len = path.len();
                path.push_str(&format!("[{index}]"));
                if let Some(difference) = first_difference(actual, expected, path) {
                    return Some(difference);
                }
                path.truncate(path_len);
            }
            None
        }
        (serde_json::Value::Object(actual), serde_json::Value::Object(expected)) => {
            if actual.len() != expected.len() {
                return Some(format!(
                    "{path}: object sizes {} != {}",
                    actual.len(),
                    expected.len()
                ));
            }
            for (name, actual) in actual {
                let Some(expected) = expected.get(name) else {
                    return Some(format!("{path}: expected object has no `{name}` field"));
                };
                let path_len = path.len();
                path.push('.');
                path.push_str(name);
                if let Some(difference) = first_difference(actual, expected, path) {
                    return Some(difference);
                }
                path.truncate(path_len);
            }
            None
        }
        _ if actual == expected => None,
        _ => Some(format!("{path}: {actual} != {expected}")),
    }
}

pub fn compare_source(path_sl: &Path, path_pl: &Path) -> Result<()> {
    let start = Instant::now();
    let (bytes_sl, payload_sl) = decode_payload(path_sl, "p4spectec.sl.v1", "sl")?;
    check_source_hash(path_sl, &bytes_sl, SOURCE_SL_SHA256)?;
    let spec_sl =
        sl::SpecCodec::decode(&payload_sl).map_err(|error| Error::Invalid(error.to_string()))?;
    drop(payload_sl);
    drop(bytes_sl);

    let num_defs = spec_sl.len();
    let spec_pl = prose::convert(spec_sl).map_err(|error| Error::Invalid(error.to_string()))?;
    let (bytes_pl, payload_pl) = decode_payload(path_pl, "p4spectec.pl.v1", "pl")?;
    check_source_hash(path_pl, &bytes_pl, SOURCE_PL_SHA256)?;
    let spec_pl_source =
        pl::SpecCodec::decode(&payload_pl).map_err(|error| Error::Invalid(error.to_string()))?;
    if spec_pl != spec_pl_source {
        let actual =
            pl::SpecCodec::encode(&spec_pl).map_err(|error| Error::Invalid(error.to_string()))?;
        let expected = pl::SpecCodec::encode(&spec_pl_source)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        let difference = first_difference(&actual, &expected, &mut "$".to_owned())
            .unwrap_or_else(|| "encoded values match but decoded PL differs".to_owned());
        return Err(Error::Invalid(format!(
            "Rust annotation differs from source PL at {difference}"
        )));
    }
    eprintln!(
        "prose-source: {num_defs} exact source-SL definitions matched source PL, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn normalize_rendered(text: &str) -> String {
    text.split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_source_spans(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                strip_source_spans(value);
            }
        }
        serde_json::Value::Object(fields) => {
            fields.remove("at");
            for value in fields.values_mut() {
                strip_source_spans(value);
            }
        }
        _ => {}
    }
}

pub fn run() -> Result<()> {
    let start = Instant::now();
    let progress = ProgressBar::new(2).with_style(
        ProgressStyle::with_template("[{bar:24}] {pos}/{len} {elapsed_precise} {msg}")
            .map_err(|error| Error::Invalid(error.to_string()))?,
    );
    progress.set_message("prose: full specification");
    let spec_el =
        parse_files([Path::new("spec")]).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_il = elaborate::convert(spec_el).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_al = algo::convert(spec_il).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_sl =
        structure::convert(spec_al, false).map_err(|error| Error::Invalid(error.to_string()))?;
    let spec_pl = prose::convert(spec_sl).map_err(|error| Error::Invalid(error.to_string()))?;
    let num_defs = spec_pl.len();

    progress.set_message("prose: rendered output");
    let rendered = normalize_rendered(&(Print::to_string(&spec_pl) + "\n"));
    let expected_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("expected");
    snapshot::check(expect_file![expected_dir.join("prose.expected")], &hash(rendered.as_bytes()));
    progress.inc(1);

    progress.set_message("prose: structured output");
    let mut structured =
        pl::SpecCodec::encode(&spec_pl).map_err(|error| Error::Invalid(error.to_string()))?;
    // The source PL is decoded and re-encoded through the Rust codec before this
    // baseline is derived. Source `iid` is therefore intentionally absent. Span
    // comparison stays in focused conversion tests because native structuring
    // already differs from the source in some inferred type-annotation spans.
    strip_source_spans(&mut structured);
    let structured =
        serde_json::to_string(&structured).map_err(|error| Error::Invalid(error.to_string()))?;
    snapshot::check(
        expect_file![expected_dir.join("prose-structure.expected")],
        &hash(structured.as_bytes()),
    );
    progress.inc(1);
    progress.finish_with_message("complete");
    eprintln!(
        "prose: {num_defs} definitions checked, elapsed={:.3}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
