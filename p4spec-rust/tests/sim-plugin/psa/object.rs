use std::collections::BTreeMap;

use num_bigint::BigInt;
use p4spec_rust::{
    lang::{
        common::source::Span,
        data::{
            typ,
            value::{Value, ValueArena, get, make},
        },
    },
    runner::{
        Extern, ExternError, Interface, InterfaceError, Interpreter, NullInterface, Runner,
        RunnerContext,
    },
    sim_plugin::{
        dummy::Dummy,
        psa::object::{Color, Counter, HashExtern, InternetChecksum, Meter, Register},
        spec_impl::{pack, unpack},
    },
};

#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error(transparent)]
    Extern(#[from] ExternError),
    #[error(transparent)]
    Interface(#[from] InterfaceError),
}

#[derive(Default)]
struct ObjectInterp {
    values_var: BTreeMap<String, Value>,
    value_default: Option<Value>,
    calls: Vec<String>,
}

impl<Iface: Interface, Exn: Extern> Interpreter<Iface, Exn> for ObjectInterp {
    type Spec = ();
    type Error = TestError;
    fn clear(&mut self) {}
    fn reset(&mut self) {}
    fn eval_program(
        _: &mut RunnerContext<'_, Self, Iface, Exn>,
        _: &str,
        _: Value,
    ) -> Result<Vec<Value>, TestError> {
        unreachable!()
    }
    fn eval_rel(
        _: &mut RunnerContext<'_, Self, Iface, Exn>,
        _: &str,
        _: &[Value],
    ) -> Result<Vec<Value>, TestError> {
        unreachable!()
    }
    fn eval_func(
        ctx: &mut RunnerContext<'_, Self, Iface, Exn>,
        name: &str,
        _: &[typ::Typ],
        values: &[Value],
    ) -> Result<Value, TestError> {
        let name_call = if name == "find_var_e" {
            let value_name = *get::case(ctx.arena(), &values[0]).unwrap().args()[0];
            get::text(ctx.arena(), &value_name).unwrap().to_owned()
        } else {
            name.to_owned()
        };
        ctx.interp_mut().calls.push(name_call.clone());
        match name {
            "find_var_e" => ctx
                .interp()
                .values_var
                .get(&name_call)
                .copied()
                .ok_or_else(|| ExternError::Failure(format!("missing {name_call}")).into()),
            "default" => Ok(ctx.interp().value_default.unwrap()),
            "find_type_e" => {
                let value_typ = ctx.interp().value_default;
                Ok(make::opt(
                    ctx.arena_mut(),
                    typ::make::opt(typ::make::text()).node.into(),
                    value_typ,
                    Span::default(),
                )
                .unwrap())
            }
            "cast_op" => Ok(values[1]),
            _ => panic!("unexpected {name}"),
        }
    }
}

type ObjectRunner = Runner<ObjectInterp, NullInterface, Dummy>;

fn setup() -> (ObjectRunner, Value, Value) {
    let mut runner = Runner::new((), ObjectInterp::default(), NullInterface, Dummy);
    let value_ctx = make::text(runner.arena_mut(), "ctx".to_owned(), Span::default()).unwrap();
    let value_arch = make::text(runner.arena_mut(), "arch".to_owned(), Span::default()).unwrap();
    runner.context().interp_mut().value_default = Some(value_ctx);
    (runner, value_ctx, value_arch)
}

fn list(arena: &mut ValueArena, values: Vec<Value>) -> Value {
    make::list(
        arena,
        typ::make::list(typ::make::text()).node.into(),
        values,
        Span::default(),
    )
    .unwrap()
}

fn arguments(arena: &mut ValueArena, args: &[(&str, Value)]) -> (Value, Value) {
    let values_name = args
        .iter()
        .map(|(name, _)| make::text(arena, (*name).to_owned(), Span::default()).unwrap())
        .collect();
    let value_ids = list(arena, values_name);
    let value_args = list(arena, args.iter().map(|(_, value)| *value).collect());
    (value_ids, value_args)
}

fn local(runner: &mut ObjectRunner, name: &str, int: i64) {
    let value = pack::p4_fixed_bit(runner.arena_mut(), 32.into(), int.into()).unwrap();
    runner
        .context()
        .interp_mut()
        .values_var
        .insert(name.to_owned(), value);
}

fn returned(arena: &ValueArena, value: Value) -> Value {
    let value_opt = *get::case(arena, &value).unwrap().args()[0];
    get::opt(arena, &value_opt).unwrap().unwrap()
}

fn tuple_data(runner: &mut ObjectRunner, int: i64) {
    let value_field = pack::p4_fixed_bit(runner.arena_mut(), 16.into(), int.into()).unwrap();
    let value_fields = list(runner.arena_mut(), vec![value_field]);
    let mixop = p4spec_rust::frontend::parse::parse_mixop("TUPLE `( value* `)").unwrap();
    let mixfix =
        p4spec_rust::lang::common::notation::mixop::Mixop::fill(&mixop, vec![value_fields])
            .unwrap();
    let typ_value = typ::make::var(
        p4spec_rust::phrase!(node: "value".to_owned(), span: Span::default()),
        vec![],
    );
    let value = make::case(
        runner.arena_mut(),
        typ_value.node.into(),
        mixfix,
        Span::default(),
    )
    .unwrap();
    runner
        .context()
        .interp_mut()
        .values_var
        .insert("data".to_owned(), value);
}

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
    let json = reg.to_json(runner.arena()).unwrap();
    let reg_decoded = Register::from_json(runner.arena_mut(), &json).unwrap();
    assert_eq!(reg_decoded.to_json(runner.arena()).unwrap(), json);
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
    let reg_decoded = Register::from_json(&mut arena_decoded, &json).unwrap();
    assert_eq!(reg_decoded.to_json(&arena_decoded).unwrap(), json);
}

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
fn test_checksum_get_updates_state_and_incremental_operations() {
    let (mut runner, value_ctx, value_arch) = setup();
    tuple_data(&mut runner, 0x1234);
    let checksum = InternetChecksum::init()
        .add(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .object;
    assert_eq!(checksum.int, 0x1234.into());
    let checksum = checksum
        .get(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .object;
    assert_eq!(checksum.int, 0xEDCB.into());
    let output = checksum
        .get_state(&mut runner.context(), value_ctx, value_arch)
        .unwrap();
    assert_eq!(
        unpack::p4_fixed_bit(
            runner.arena(),
            &returned(runner.arena(), output.result.value_call_result)
        )
        .unwrap()
        .int,
        0xEDCB.into()
    );
    let checksum = output
        .object
        .get(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .object;
    assert_eq!(checksum.int, 0x1234.into());
    let checksum = checksum
        .subtract(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .object;
    assert_eq!(checksum.int, 65535.into());
    local(&mut runner, "checksum_state", 7);
    let checksum = checksum
        .set_state(&mut runner.context(), value_ctx, value_arch)
        .unwrap()
        .object;
    assert_eq!(checksum.int, 7.into());
    assert_eq!(
        checksum
            .clear(&mut runner.context(), value_ctx, value_arch)
            .unwrap()
            .object
            .int,
        0.into()
    );
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
