use super::*;

#[test]
fn test_checksum_get_updates_state_and_incremental_operations() {
    let (mut runner, value_ctx, value_arch) = setup();
    tuple_data(&mut runner, 0x1234);
    let checksum = InternetChecksum::init()
        .add(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .0;
    assert_eq!(checksum.int, 0x1234.into());
    let checksum = checksum
        .get(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .0;
    assert_eq!(checksum.int, 0xEDCB.into());
    let output = checksum
        .get_state(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(
        unpack::p4_fixed_bit(runner.arena(), &returned(runner.arena(), output.3))
            .unwrap()
            .1,
        0xEDCB.into()
    );
    let checksum = output
        .0
        .get(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .0;
    assert_eq!(checksum.int, 0x1234.into());
    let checksum = checksum
        .subtract(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .0;
    assert_eq!(checksum.int, 65535.into());
    local(&mut runner, "checksum_state", 7);
    let checksum = checksum
        .set_state(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .0;
    assert_eq!(checksum.int, 7.into());
    assert_eq!(
        checksum
            .clear(&mut runner.context(), value_ctx, value_arch)
            .unwrap()
            .0
            .int,
        0.into()
    );
}
