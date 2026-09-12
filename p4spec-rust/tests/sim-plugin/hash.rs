use num_bigint::BigInt;
use p4spec_rust::{
    frontend::parse::parse_mixop,
    lang::{
        common::{notation::mixop::Mixop, source::Span},
        data::{
            typ,
            value::{Value, ValueArena, make},
        },
    },
    sim_plugin::{
        hash,
        spec_impl::{pack, unpack::PrecisionNumber},
    },
};

#[test]
fn test_hashes_match_ocaml_vectors() {
    // Captured from the pinned backend-sim/hash.ml functions
    for (width, int, int_init, ints_expect) in [
        (0, 0_u64, 0, [0_u64, 0, 65535, 65535, 0]),
        (16, 10, 0, [1920, 2701982689, 65525, 10, 10]),
        (16, 4660, 0, [30477, 412718745, 60875, 4660, 4660]),
        (
            32,
            305419896,
            0,
            [13435, 1242107544, 38739, 26796, 305419896],
        ),
        (32, 4294967295, 0, [37889, 4294967295, 0, 65535, 4294967295]),
        (16, 4660, 65280, [30477, 412718745, 61130, 4915, 4660]),
    ] {
        let bits = PrecisionNumber {
            width: width.into(),
            int: int.into(),
        };
        for (algo, int_expect) in ["crc16", "crc32", "csum16", "csum16_sub", "identity"]
            .into_iter()
            .zip(ints_expect)
        {
            assert_eq!(
                hash::compute_hash(algo, Some(&int_init.into()), &bits).unwrap(),
                int_expect.into(),
                "{algo}: {bits:?}"
            );
        }
    }
}

fn precision(arena: &mut ValueArena, shape: &str, ints: &[i64]) -> Value {
    let values: Vec<_> = ints
        .iter()
        .enumerate()
        .map(|(idx, int)| {
            if idx + 1 < ints.len() {
                make::nat(
                    arena,
                    BigInt::from(*int).try_into().unwrap(),
                    Span::default(),
                )
                .unwrap()
            } else {
                make::int(arena, (*int).into(), Span::default()).unwrap()
            }
        })
        .collect();
    let mixop = parse_mixop(shape).unwrap();
    let value_case = Mixop::fill(&mixop, values).unwrap();
    let typ = typ::make::var(
        p4spec_rust::phrase!(node: "value".to_owned(), span: Span::default()),
        Vec::new(),
    );
    make::case(arena, typ.node.into(), value_case, Span::default()).unwrap()
}

#[test]
fn test_package_normalizes_signed_fields_and_pads_without_shifting() {
    let mut arena = ValueArena::new();
    let value_nibble = pack::p4_fixed_bit(&mut arena, 4.into(), 10.into()).unwrap();
    let bits = hash::package(&arena, &[value_nibble]).unwrap();
    assert_eq!((bits.width, bits.int), (16.into(), 10.into()));
    assert_eq!(
        hash::compute_checksum("crc16", None, &arena, &[value_nibble]).unwrap(),
        1920.into()
    );
    let value_signed = precision(&mut arena, "nat S int", &[8, -1]);
    let value_var = precision(&mut arena, "nat '.' nat V int", &[32, 4, 5]);
    let bits = hash::package(&arena, &[value_nibble, value_signed, value_var]).unwrap();
    assert_eq!((bits.width, bits.int), (16.into(), 0xAFF5.into()));
    let bits = hash::package(&arena, &[value_var, value_signed, value_nibble]).unwrap();
    assert_eq!(bits.int, 0x5FFA.into());
    let value_over = pack::p4_fixed_bit(&mut arena, 4.into(), 26.into()).unwrap();
    assert_eq!(hash::package(&arena, &[value_over]).unwrap().int, 10.into());
    let bits = hash::package(&arena, &[]).unwrap();
    assert_eq!((bits.width, bits.int), (0.into(), 0.into()));
}

#[test]
fn test_hash_width_range_and_complement_boundaries() {
    assert_eq!(
        hash::bitwise_neg(&74565.into(), &8.into()).unwrap(),
        74682.into()
    );
    assert_eq!(
        hash::bitwise_neg(&74565.into(), &0.into()).unwrap(),
        74565.into()
    );
    assert_eq!(
        hash::bitwise_neg(&74565.into(), &(-1).into()).unwrap(),
        74565.into()
    );
    assert_eq!(
        hash::adjust(&5.into(), &12.into(), &20.into()).unwrap(),
        11.into()
    );
    assert_eq!(
        hash::adjust(&5.into(), &0.into(), &20.into()).unwrap(),
        5.into()
    );
    assert!(hash::adjust(&5.into(), &5.into(), &20.into()).is_err());
    assert!(hash::adjust(&5.into(), &3.into(), &20.into()).is_err());
    assert_eq!(
        hash::adjust(&5.into(), &12.into(), &(-20).into()).unwrap(),
        6.into()
    );
    for (int_init, int_sum, int_sub) in [(-65537, 60876, 4661), (131072, 60874, 4659)] {
        let bits = PrecisionNumber {
            width: 16.into(),
            int: 4660.into(),
        };
        assert_eq!(
            hash::compute_hash("csum16", Some(&int_init.into()), &bits).unwrap(),
            int_sum.into()
        );
        assert_eq!(
            hash::compute_hash("csum16_sub", Some(&int_init.into()), &bits).unwrap(),
            int_sub.into()
        );
    }
    for algo in ["crc16", "crc32", "csum16", "csum16_sub"] {
        assert!(
            hash::compute_hash(
                algo,
                None,
                &PrecisionNumber {
                    width: 4.into(),
                    int: 10.into()
                }
            )
            .is_err()
        );
    }
    let bits = PrecisionNumber {
        width: (-1).into(),
        int: BigInt::from(123),
    };
    assert_eq!(
        hash::compute_hash("identity", None, &bits).unwrap(),
        bits.int
    );
    assert!(hash::compute_hash("unsupported", None, &bits).is_err());
    let mut arena = ValueArena::new();
    let value_bad = precision(&mut arena, "nat W int", &[4, 10]);
    let value_bad = make::list(
        &mut arena,
        typ::make::list(typ::make::text()).node.into(),
        vec![value_bad],
        Span::default(),
    )
    .unwrap();
    assert!(hash::package(&arena, &[value_bad]).is_err());
}
