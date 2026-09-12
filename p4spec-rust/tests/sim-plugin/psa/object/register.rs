use super::*;
use p4spec_rust::lang::data::value::serde::{decode, encode};

#[test]
fn test_register_default_order_read_bounds_and_write_noop() {
    let (mut runner, value_ctx, value_arch) = setup();
    let value_targs = list(runner.arena_mut(), vec![value_ctx, value_arch]);
    let value_bad_size = make::bool(runner.arena_mut(), false, Span::default()).unwrap();
    let (value_ids, value_args) = arguments(runner.arena_mut(), &[("size", value_bad_size)]);
    assert!(Register::init(&mut runner.context(), value_targs, value_ids, value_args).is_err());
    assert_eq!(runner.context().interp().calls, ["default"]);
    let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), 2.into()).unwrap();
    let (value_ids, value_args) = arguments(runner.arena_mut(), &[("size", value_size)]);
    let reg = Register::init(&mut runner.context(), value_targs, value_ids, value_args).unwrap();
    assert_eq!(reg.values, [value_ctx, value_ctx]);
    runner.context().interp_mut().calls.clear();
    let (value_ids, value_args) = arguments(
        runner.arena_mut(),
        &[("size", value_size), ("initial_value", value_arch)],
    );
    let reg_initial =
        Register::init(&mut runner.context(), value_targs, value_ids, value_args).unwrap();
    assert_eq!(reg_initial.values, [value_arch, value_arch]);
    assert!(runner.context().interp().calls.is_empty());
    local(&mut runner, "index", -1);
    runner.context().interp_mut().calls.clear();
    assert!(
        reg.clone()
            .read(&mut runner.context(), value_ctx, value_arch)
            .is_err()
    );
    assert_eq!(runner.context().interp().calls, ["index"]);
    local(&mut runner, "index", 3);
    let output = reg
        .clone()
        .read(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(
        returned(runner.arena(), output.result.value_call_result),
        value_ctx
    );
    runner
        .context()
        .interp_mut()
        .values_var
        .insert("value".to_owned(), value_arch);
    assert_eq!(
        reg.clone()
            .write(&mut runner.context(), value_ctx, value_arch)
            .unwrap()
            .object,
        reg
    );
    local(&mut runner, "index", 1);
    let reg = reg
        .write(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .object;
    assert_eq!(reg.values, [value_ctx, value_arch]);
    let json = encode(runner.arena(), &reg).unwrap();
    let reg_decoded: Register = decode(runner.arena_mut(), &json).unwrap();
    assert_eq!(encode(runner.arena(), &reg_decoded).unwrap(), json);
    assert_eq!(
        runner.arena().typ(&reg_decoded.value_typ),
        runner.arena().typ(&reg.value_typ)
    );
    for (value_decoded, value) in reg_decoded.values.iter().zip(&reg.values) {
        assert_eq!(runner.arena().typ(value_decoded), runner.arena().typ(value));
        assert_eq!(
            runner.arena().span(value_decoded),
            runner.arena().span(value)
        );
        assert_eq!(
            get::text(runner.arena(), value_decoded),
            get::text(runner.arena(), value)
        );
    }
    let mut arena_decoded = ValueArena::new();
    let reg_decoded: Register = decode(&mut arena_decoded, &json).unwrap();
    assert_eq!(encode(&arena_decoded, &reg_decoded).unwrap(), json);
}
