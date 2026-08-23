use p4spec_rust::{
    domain::source::{Region, Spanned},
    lang::{
        el::ast::{self, ExpKind, Hole},
        hints::{fields, flag, input},
        xl::{utf8, var},
    },
};

fn span(name: &str) -> Region {
    Region::for_file(name)
}

fn id(name: &str, source: &str) -> ast::Id {
    Spanned::new(name.to_owned(), span(source))
}

fn exp(node: ExpKind) -> ast::Exp {
    Spanned::new(node, span("hint"))
}

#[test]
fn utf8_encodes_and_decodes_valid_codepoints() {
    let codepoints = [0x24, 0xa2, 0x20ac, 0x10348];
    let bytes = utf8::encode(&codepoints).unwrap();

    assert_eq!(
        bytes,
        vec![0x24, 0xc2, 0xa2, 0xe2, 0x82, 0xac, 0xf0, 0x90, 0x8d, 0x88]
    );
    assert_eq!(utf8::decode(&bytes).unwrap(), codepoints);
}

#[test]
fn utf8_encoder_accepts_surrogates_while_decoder_rejects_them() {
    let surrogate = utf8::encode(&[0xd800]).unwrap();

    assert_eq!(surrogate, vec![0xed, 0xa0, 0x80]);
    assert!(utf8::decode(&surrogate).is_err());
}

#[test]
fn utf8_rejects_invalid_codepoints_and_byte_sequences() {
    assert!(utf8::encode(&[-1]).is_err());
    assert!(utf8::encode(&[0x110000]).is_err());

    for bytes in [
        &[0xc0, 0x80][..],             // overlong NUL
        &[0xed, 0xa0, 0x80][..],       // surrogate U+D800
        &[0xf4, 0x90, 0x80, 0x80][..], // above U+10FFFF
        &[0xe2, 0x28, 0xa1][..],       // invalid continuation
        &[0xf0, 0x90, 0x80][..],       // truncated sequence
    ] {
        assert!(utf8::decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}

#[test]
fn strip_var_suffix_keeps_source_region_and_only_preserves_all_underscores() {
    let suffixed = id("value_suffix", "suffix-source");
    let apostrophe = id("value'", "apostrophe-source");
    let all_underscores = id("value___", "underscore-source");

    let stripped = var::strip_var_suffix(&suffixed);
    assert_eq!(stripped.node, "value");
    assert_eq!(stripped.span, suffixed.span);
    assert_eq!(var::strip_var_suffix(&apostrophe).node, "value");
    assert_eq!(var::strip_var_suffix(&all_underscores).node, "value___");
}

#[test]
fn input_hints_initialize_validate_split_combine_and_detect_conditional_relations() {
    let sequence = exp(ExpKind::SeqE(vec![
        exp(ExpKind::HoleE(Hole::Num(2))),
        exp(ExpKind::HoleE(Hole::Num(0))),
    ]));
    assert_eq!(input::init(&sequence), Some(vec![2, 0]));
    assert_eq!(
        input::init(&exp(ExpKind::HoleE(Hole::Num(1)))),
        Some(vec![1])
    );
    assert_eq!(input::init(&exp(ExpKind::HoleE(Hole::Next))), None);

    assert!(input::validate(&[], 3).is_err());
    assert!(input::validate(&[1, 1], 3).is_err());
    assert!(input::validate(&[-1], 3).is_err());
    assert!(input::validate(&[3], 3).is_err());
    assert_eq!(input::validate(&[2, 0], 3), Ok(()));

    let items = ["zero", "one", "two", "three"];
    let hint = vec![2, 0];
    let (items_input, items_output) = input::split(&hint, &items);
    assert_eq!(items_input, vec!["zero", "two"]);
    assert_eq!(items_output, vec!["one", "three"]);
    assert_eq!(input::combine(&hint, items_input, items_output), items);
    assert!(input::is_conditional(&[0, 1], &["left", "right"]));
    assert!(!input::is_conditional(&[0], &["left", "right"]));
}

#[test]
fn fields_hints_initialize_from_text_and_require_exact_arity() {
    let single = exp(ExpKind::TextE("left".to_owned()));
    let sequence = exp(ExpKind::SeqE(vec![
        exp(ExpKind::TextE("left".to_owned())),
        exp(ExpKind::TextE("right".to_owned())),
    ]));

    assert_eq!(fields::init(&single), Some(vec!["left".to_owned()]));
    assert_eq!(
        fields::init(&sequence),
        Some(vec!["left".to_owned(), "right".to_owned()])
    );
    assert_eq!(fields::init(&exp(ExpKind::HoleE(Hole::Next))), None);
    assert_eq!(fields::validate(&["left".to_owned()], 1), Ok(()));
    assert!(fields::validate(&["left".to_owned()], 2).is_err());
}

#[test]
fn flag_hints_match_only_the_requested_hint_identifier() {
    let hints = vec![ast::Hint {
        hintid: id("enabled", "enabled-hint"),
        hintexp: exp(ExpKind::EpsE),
    }];

    assert!(flag::init(&hints, "enabled"));
    assert!(!flag::init(&hints, "disabled"));
}
