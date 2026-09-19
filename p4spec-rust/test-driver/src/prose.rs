use crate::{Error, Result, snapshot};
use expect_test::expect_file;
use indicatif::{ProgressBar, ProgressStyle};
use p4spec_rust::{
    frontend::parse::parse_files,
    lang::traits::print::Print,
    pass::{algo, elaborate, prose, structure},
    wire::ocaml::lang::pl::SpecCodec,
};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Instant};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}\n", Sha256::digest(bytes))
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
        SpecCodec::encode(&spec_pl).map_err(|error| Error::Invalid(error.to_string()))?;
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
