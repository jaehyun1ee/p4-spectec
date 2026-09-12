use super::*;

#[test]
fn test_meter_returns_green_without_reading_inputs() {
    let (mut runner, value_ctx, value_arch) = setup();
    let meter = Meter::Bytes(vec![Color::Red]);
    for output in [
        meter
            .clone()
            .execute_color_aware(&mut runner.context(), value_ctx, value_arch)
            .unwrap(),
        meter
            .clone()
            .execute_color_blind(&mut runner.context(), value_ctx, value_arch)
            .unwrap(),
    ] {
        assert_eq!(output.object, meter);
        assert_eq!(
            unpack::p4_enum(
                runner.arena(),
                &returned(runner.arena(), output.result.value_call_result)
            )
            .unwrap(),
            ("PSA_MeterColor_t".to_owned(), "GREEN".to_owned())
        );
    }
    assert!(runner.context().interp().calls.is_empty());
    let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), 2.into()).unwrap();
    let value_type = pack::p4_enum(runner.arena_mut(), "PSA_MeterType_t", "PACKETS").unwrap();
    let (value_ids, value_args) = arguments(
        runner.arena_mut(),
        &[("n_meters", value_size), ("type", value_type)],
    );
    assert_eq!(
        Meter::init(runner.arena(), value_ctx, value_ids, value_args).unwrap(),
        Meter::Packets(vec![Color::Green; 2])
    );
}
