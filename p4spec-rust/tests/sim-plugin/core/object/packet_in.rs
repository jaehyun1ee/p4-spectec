use super::*;

#[test]
fn test_successive_extracts_and_short_rejection_preserve_state() {
    let (mut runner, value_ctx, value_arch) = packet_runner(4, 4);
    let pkt = PacketIn::init("aB").unwrap();
    let output_a = pkt
        .extract(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(output_a.pkt.idx, 4);
    assert_eq!(
        bits(runner.arena(), &output_a.result.value_ctx),
        [true, false, true, false]
    );
    let output_b = output_a
        .pkt
        .extract(&mut runner.context(), output_a.result.value_ctx, value_arch)
        .unwrap();
    assert_eq!(output_b.pkt.idx, 8);
    assert_eq!(
        bits(runner.arena(), &output_b.result.value_ctx),
        [true, false, true, true]
    );
    let output_c = output_b
        .pkt
        .extract(&mut runner.context(), output_b.result.value_ctx, value_arch)
        .unwrap();
    assert_eq!(output_c.pkt, output_b.pkt);
    assert_eq!(output_c.result.value_ctx, output_b.result.value_ctx);
    assert_eq!(output_c.result.value_arch, value_arch);
    assert_eq!(
        reject(runner.arena(), &output_c.result.value_call_result),
        "PacketTooShort"
    );
    let ctx = runner.context();
    let calls = &ctx.interp().calls;
    let (_, values) = calls
        .iter()
        .find(|(name, _)| name == "write_value_from_bits")
        .unwrap();
    assert_eq!(
        num::to_int(get::num(ctx.arena(), &values[1]).unwrap()).to_string(),
        "0"
    );
    assert!(
        matches!(ctx.arena().typ(&values[2]).as_ref(), TypKind::Iter(typ, _) if matches!(&typ.node, TypKind::Var(id, _) if id.node == "bit"))
    );
}

#[test]
fn test_lookahead_keeps_cursor_and_defaults_before_short_check() {
    let (mut runner, value_ctx, value_arch) = packet_runner(4, 4);
    let pkt = PacketIn::init("A").unwrap();
    let output = pkt
        .lookahead(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(output.pkt, pkt);
    let value_opt = *get::case(runner.arena(), &output.result.value_call_result)
        .unwrap()
        .args()[0];
    let value_hdr = get::opt(runner.arena(), &value_opt).unwrap().unwrap();
    assert_eq!(bits(runner.arena(), &value_hdr), [true, false, true, false]);
    runner.context().interp_mut().calls.clear();
    runner.context().interp_mut().size_max = 8;
    let output = pkt
        .lookahead(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(
        reject(runner.arena(), &output.result.value_call_result),
        "PacketTooShort"
    );
    let names: Vec<_> = runner
        .context()
        .interp()
        .calls
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    assert_eq!(
        names,
        [
            "find_type_e",
            "subst_type_e",
            "sizeof_maxSizeInBits'",
            "default"
        ]
    );
}

#[test]
fn test_variable_extract_checks_alignment_then_packet_then_header() {
    let (mut runner, value_ctx, value_arch) = packet_runner(0, 4);
    let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), 8.into()).unwrap();
    runner
        .context()
        .interp_mut()
        .values_var
        .insert("variableFieldSizeInBits".to_owned(), value_size);
    for (alignment, packet, signal) in [
        (1, "A", "ParserInvalidArgument"),
        (0, "A", "PacketTooShort"),
        (0, "AB", "HeaderTooShort"),
    ] {
        runner.context().interp_mut().alignment = alignment;
        let pkt = PacketIn::init(packet).unwrap();
        let output = pkt
            .extract_varsize(&mut runner.context(), value_ctx, value_arch)
            .unwrap();
        assert_eq!(output.pkt, pkt);
        assert_eq!(output.result.value_ctx, value_ctx);
        assert_eq!(
            reject(runner.arena(), &output.result.value_call_result),
            signal
        );
    }
    runner.context().interp_mut().size_max = 8;
    let output = PacketIn::init("AB")
        .unwrap()
        .extract_varsize(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(output.pkt.idx, 8);
    let ctx = runner.context();
    let (_, values) = ctx
        .interp()
        .calls
        .iter()
        .rev()
        .find(|(name, _)| name == "write_value_from_bits")
        .unwrap();
    assert_eq!(
        num::to_int(get::num(ctx.arena(), &values[1]).unwrap()).to_string(),
        "8"
    );
}

#[test]
fn test_malformed_or_failed_write_does_not_install_partial_packet_or_context() {
    let (mut runner, value_ctx, value_arch) = packet_runner(4, 4);
    let pkt = PacketIn::init("A").unwrap();
    runner.context().interp_mut().arity_rel = 2;
    let error = pkt
        .extract(&mut runner.context(), value_ctx, value_arch)
        .unwrap_err();
    assert!(matches!(
        error,
        TestError::Extern(ExternError::Value(ValueError::ExpectedCount {
            expected: 1,
            actual: 2
        }))
    ));
    assert_eq!(pkt.idx, 0);
    assert_eq!(get::text(runner.arena(), &value_ctx), Ok("ctx"));
    runner.context().interp_mut().fail_rel = true;
    assert!(
        matches!(pkt.extract(&mut runner.context(), value_ctx, value_arch), Err(TestError::Extern(ExternError::Failure(message))) if message == "write failed")
    );
    assert_eq!(pkt.idx, 0);
    runner.context().interp_mut().fail_rel = false;
    runner.context().interp_mut().arity_rel = 1;
    runner.context().interp_mut().value_typ = None;
    assert!(
        pkt.extract(&mut runner.context(), value_ctx, value_arch)
            .is_err()
    );
}

#[test]
fn test_packet_json_validates_cursor_and_preserves_payload() {
    let pkt = PacketIn::init("a").unwrap();
    let json = serde_json::to_value(&pkt).unwrap();
    let mut json_short = json.clone();
    json_short["len"] = 2.into();
    let pkt_short = PacketIn::from_json(&json_short).unwrap();
    assert_eq!(pkt_short.bits.len(), 4);
    assert_eq!(pkt_short.payload().unwrap(), [true, false]);
    for (name, json_bad) in [
        ("idx", serde_json::json!(-1)),
        ("idx", serde_json::json!(5)),
        ("len", serde_json::json!(5)),
        ("len", serde_json::json!(1_u64 << 62)),
        ("bits", serde_json::json!([1])),
    ] {
        let mut json_invalid = json.clone();
        json_invalid[name] = json_bad;
        assert!(PacketIn::from_json(&json_invalid).is_err());
    }
    assert!(PacketIn::init("G").is_err());
    assert_eq!(object::bits_to_string(&[true, false, true]), "A");
    let (mut pkt, _) = pkt.parse(2).unwrap();
    pkt.reset();
    assert_eq!(pkt.idx, 0);
    assert_eq!(
        object::bits_to_int_signed(&[true, false, true]).unwrap(),
        BigInt::from(-3)
    );
    assert_eq!(
        object::int_to_bits_signed(&BigInt::from(-3), 3),
        [true, false, true]
    );
    assert!(object::bits_to_int_signed(&[]).is_err());
}

#[test]
fn test_packet_size_conversion_rejects_negative_and_ocaml_overflow() {
    let (mut runner, value_ctx, value_arch) = packet_runner(0, 0);
    let pkt = PacketIn::init("A").unwrap();
    for int in [BigInt::from(-1), BigInt::from(1) << 62] {
        let value_size = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), int).unwrap();
        runner
            .context()
            .interp_mut()
            .values_var
            .insert("sizeInBits".to_owned(), value_size);
        assert!(
            pkt.advance(&mut runner.context(), value_ctx, value_arch)
                .is_err()
        );
        assert_eq!(pkt.idx, 0);
    }
}
