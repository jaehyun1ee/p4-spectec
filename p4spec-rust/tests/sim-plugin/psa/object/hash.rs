use super::*;

#[test]
fn test_hash_adjust_uses_max_not_range_and_preserves_call_order() {
    let (mut runner, value_ctx, value_arch) = setup();
    tuple_data(&mut runner, 20);
    local(&mut runner, "base", 5);
    local(&mut runner, "max", 12);
    let hash = HashExtern {
        algo: "identity".to_owned(),
    };
    let output = hash
        .clone()
        .get_hash(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    let value = returned(runner.arena(), output.result.value_call_result);
    let value_int = *get::case(runner.arena(), &value).unwrap().args()[0];
    assert_eq!(
        p4spec_rust::lang::xl::num::to_int(get::num(runner.arena(), &value_int).unwrap()),
        &20.into()
    );
    assert_eq!(
        runner.context().interp().calls,
        ["data", "find_type_e", "cast_op"]
    );
    runner.context().interp_mut().calls.clear();
    let output = hash
        .clone()
        .get_hash_adjust(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    let value = returned(runner.arena(), output.result.value_call_result);
    let value_int = *get::case(runner.arena(), &value).unwrap().args()[0];
    assert_eq!(
        p4spec_rust::lang::xl::num::to_int(get::num(runner.arena(), &value_int).unwrap()),
        &13.into()
    );
    assert_eq!(
        runner.context().interp().calls,
        ["base", "max", "data", "find_type_e", "cast_op"]
    );
    local(&mut runner, "max", 0);
    runner.context().interp_mut().calls.clear();
    assert!(
        hash.get_hash_adjust(&mut runner.context(), value_ctx, value_arch)
            .is_err()
    );
    assert_eq!(runner.context().interp().calls, ["base", "max", "data"]);
}

#[test]
fn test_hash_constructor_maps_known_algorithms_and_keeps_unknown_spelling() {
    let (mut runner, value_ctx, _) = setup();
    for (id, algo) in [
        ("IDENTITY", "identity"),
        ("CRC32", "crc32"),
        ("CRC16", "crc16"),
        ("ONES_COMPLEMENT16", "csum16"),
        ("TARGET_CUSTOM", "TARGET_CUSTOM"),
    ] {
        let value_algo = pack::p4_enum(runner.arena_mut(), "PSA_HashAlgorithm_t", id).unwrap();
        let (value_ids, value_args) = arguments(runner.arena_mut(), &[("algo", value_algo)]);
        let hash = HashExtern::init(runner.arena(), value_ctx, value_ids, value_args).unwrap();
        assert_eq!(hash.algo, algo);
    }
}
