use super::*;

#[test]
fn test_advance_length_emit_and_payload_keep_bit_order() {
    let (mut runner, value_ctx, value_arch) = packet_runner(4, 4);
    let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), 3.into()).unwrap();
    runner
        .context()
        .interp_mut()
        .values_var
        .insert("sizeInBits".to_owned(), value_size);
    let pkt = PacketIn::init("ABC").unwrap();
    let output = pkt
        .advance(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(output.0.idx, 3);
    assert_eq!(output.0.payload_bytes().unwrap(), [BigInt::from(94)]);
    let output_len = output
        .0
        .length(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    let value_opt = *get::case(runner.arena(), &output_len.3).unwrap().args()[0];
    let value_len = get::opt(runner.arena(), &value_opt).unwrap().unwrap();
    let values = get::case(runner.arena(), &value_len).unwrap().args();
    assert_eq!(
        num::to_int(get::num(runner.arena(), values[1]).unwrap()).to_string(),
        "2"
    );
    let values_bits = [false, true]
        .into_iter()
        .map(|bit| make::bool(runner.arena_mut(), bit, Span::default()).unwrap())
        .collect();
    let value_hdr = make::list(
        runner.arena_mut(),
        typ::make::list(typ::make::bool()).node.into(),
        values_bits,
        Span::default(),
    )
    .unwrap();
    runner
        .context()
        .interp_mut()
        .values_var
        .insert("hdr".to_owned(), value_hdr);
    let output_emit = PacketOut::default()
        .emit(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(
        object::packet::to_string(&output.0, &output_emit.0).unwrap(),
        "578"
    );
    assert_eq!(output_emit.1, value_ctx);
    assert_eq!(output_emit.2, value_arch);
    let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), 10.into()).unwrap();
    runner
        .context()
        .interp_mut()
        .values_var
        .insert("sizeInBits".to_owned(), value_size);
    let output_short = output
        .0
        .advance(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(output_short.0, output.0);
    assert_eq!(reject(runner.arena(), &output_short.3), "PacketTooShort");
}
