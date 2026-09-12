use super::*;

#[test]
fn test_counter_source_variants_bigints_and_packets_only_count() {
    let (mut runner, value_ctx, value_arch) = setup();
    for (id, counter_expect) in [
        ("PACKETS", Counter::Packets(vec![0.into(); 2])),
        ("BYTES", Counter::Bytes(vec![0.into(); 2])),
        (
            "PACKETS_AND_BYTES",
            Counter::PacketsAndBytes(vec![(0.into(), 0.into()); 2]),
        ),
    ] {
        let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), 2.into()).unwrap();
        let value_type = pack::p4_enum(runner.arena_mut(), "PSA_CounterType_t", id).unwrap();
        let (value_ids, value_args) = arguments(
            runner.arena_mut(),
            &[("n_counters", value_size), ("type", value_type)],
        );
        let counter = Counter::init(runner.arena(), value_ctx, value_ids, value_args).unwrap();
        assert_eq!(counter, counter_expect);
        let json = serde_json::to_value(&counter).unwrap();
        let counter: Counter = serde_json::from_value(json).unwrap();
        local(&mut runner, "index", 1);
        let result = counter.count(&mut runner.context(), value_ctx, value_arch);
        if id == "PACKETS" {
            let output = result.unwrap();
            assert_eq!(output.object, Counter::Packets(vec![0.into(), 1.into()]));
            assert_eq!(output.result.value_arch, value_arch);
        } else {
            assert!(result.is_err());
        }
    }
    let int = BigInt::from(1) << 100;
    let counter = Counter::Packets(vec![int]);
    local(&mut runner, "index", -1);
    assert_eq!(
        counter
            .clone()
            .count(&mut runner.context(), value_ctx, value_arch)
            .unwrap()
            .object,
        counter
    );
    let json = serde_json::to_value(&counter).unwrap();
    assert_eq!(serde_json::from_value::<Counter>(json).unwrap(), counter);
}
