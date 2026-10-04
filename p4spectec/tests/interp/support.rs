use p4spectec::{
    frontend::parse::parse_text,
    interp::sl::SlInterp,
    pass::{algo, elaborate, structure},
    runner::{self, BuiltinInterface, Config, NullExtern, Runner},
};

/// Builds a checked SL runner from an in-memory specification.
pub fn sl_runner(source: &str) -> Runner<SlInterp, BuiltinInterface, NullExtern> {
    let spec_sl = sl_spec(source);
    runner::build_sl(spec_sl, Config::new(false, false, true), NullExtern).unwrap()
}

/// Converts an in-memory specification through the checked SL passes.
pub fn sl_spec(source: &str) -> p4spectec::lang::sl::ast::Spec {
    let spec_el = parse_text("assignment-test".into(), source).unwrap();
    let spec_il = elaborate::convert(spec_el).unwrap();
    let spec_al = algo::convert(spec_il).unwrap();
    structure::convert(spec_al, true).unwrap()
}
