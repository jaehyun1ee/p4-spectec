use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use p4spec_rust::{
    lang::common::{
        notation::{atom::Atom, mixfix::Mixfix, mixop::Mixop},
        source::{NotePhrase, Position, Span},
    },
    note_phrase, phrase,
};

fn atom(node: Atom) -> p4spec_rust::lang::common::notation::mixfix::AtomPhrase {
    phrase!(node: node, span: Span::default())
}

fn span(line: i64) -> Span {
    Span::new(
        Position::new("notation.spec", line, 2),
        Position::new("notation.spec", line, 5),
    )
}

fn hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn test_map_span_preserves_nested_labels_arguments_and_locations() {
    let arg_l: NotePhrase<u32, u8, u8> = note_phrase!(node: 17, note: 3_u8, span: 7_u8);
    let arg_r: NotePhrase<u32, u8, u8> = note_phrase!(node: 29, note: 5_u8, span: 9_u8);
    let mixfix = Mixfix::Brack(
        phrase!(node: Atom::LParen, span: span(11)),
        Box::new(Mixfix::Infix(
            Box::new(Mixfix::Arg(arg_l)),
            phrase!(node: Atom::Arrow, span: span(13)),
            Box::new(Mixfix::Seq(vec![
                Mixfix::Atom(phrase!(node: Atom::Keyword("tail".to_owned()), span: span(17))),
                Mixfix::Arg(arg_r),
            ])),
        )),
        phrase!(node: Atom::RParen, span: span(19)),
    );
    let mut spans = Vec::new();
    let mixfix_tokens: Mixfix<_, u8> = mixfix.map_span(|span| {
        let index = u8::try_from(spans.len()).unwrap();
        spans.push(span);
        index
    });

    assert_eq!(spans, vec![span(11), span(13), span(17), span(19)]);
    assert_eq!(mixfix_tokens.args(), vec![&arg_l, &arg_r]);
    assert_eq!(
        mixfix_tokens
            .atoms()
            .into_iter()
            .map(|atom| atom.span)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3],
    );

    let mixfix_restored = mixfix_tokens.map_span(|index| spans[usize::from(index)].clone());
    assert!(matches!(&mixfix_restored, Mixfix::Brack(_, mixfix, _)
        if matches!(mixfix.as_ref(), Mixfix::Infix(_, _, mixfix)
            if matches!(mixfix.as_ref(), Mixfix::Seq(_)))));
    assert_eq!(
        mixfix_restored
            .atoms()
            .into_iter()
            .map(|atom| (&atom.node, &atom.span))
            .collect::<Vec<_>>(),
        vec![
            (&Atom::LParen, &span(11)),
            (&Atom::Arrow, &span(13)),
            (&Atom::Keyword("tail".to_owned()), &span(17)),
            (&Atom::RParen, &span(19)),
        ],
    );
    assert_eq!(mixfix_restored.into_args(), vec![arg_l, arg_r]);
}

#[test]
fn test_span_mapping_preserves_syntax_comparisons_and_hashing() {
    let mixfix = Mixfix::Infix(
        Box::new(Mixfix::Arg(17)),
        phrase!(node: Atom::Arrow, span: span(11)),
        Box::new(Mixfix::Arg(29)),
    );
    let mixfix_relocated = mixfix.clone().map_span(|_| span(37));
    let mixfix_tokens = mixfix.clone().map_span(|_| 3_u8);

    assert_eq!(mixfix, mixfix_relocated);
    assert_eq!(mixfix.cmp(&mixfix_relocated), Ordering::Equal);
    assert_eq!(hash(&mixfix), hash(&mixfix_relocated));
    assert_eq!(hash(&mixfix), hash(&mixfix_tokens));
    assert!(mixfix.eq_shape(&mixfix_tokens));
    assert!(mixfix.eq_by(&mixfix_tokens, PartialEq::eq));
    assert_eq!(mixfix.cmp_by(&mixfix_tokens, Ord::cmp), Ordering::Equal);

    let (mixop, args) = mixfix_tokens.split();
    let mixfix_filled = Mixop::fill(&mixop, args.into_iter().copied()).unwrap();
    assert_eq!(mixfix_filled, mixfix_tokens);
    assert_eq!(mixfix_filled.atoms()[0].span, 3);

    let mixfix_changed = mixfix_tokens.map(|arg| arg + 1);
    assert!(!mixfix.eq_by(&mixfix_changed, PartialEq::eq));
    assert_eq!(mixfix.cmp_by(&mixfix_changed, Ord::cmp), Ordering::Less);
}

#[test]
fn test_into_args_preserves_nested_argument_order() {
    let mixfix = Mixfix::Seq(vec![
        Mixfix::Brack(
            atom(Atom::LParen),
            Box::new(Mixfix::Arg(1)),
            atom(Atom::RParen),
        ),
        Mixfix::Infix(
            Box::new(Mixfix::Arg(2)),
            atom(Atom::Arrow),
            Box::new(Mixfix::Seq(vec![Mixfix::Arg(3), Mixfix::Arg(4)])),
        ),
    ]);

    let args = mixfix.into_args();

    assert_eq!(args, vec![1, 2, 3, 4]);
}

#[test]
fn test_eq_shape_borrows_nested_mixfixes_and_ignores_arguments() {
    let left = Mixfix::Infix(
        Box::new(Mixfix::Arg(1)),
        atom(Atom::Arrow),
        Box::new(Mixfix::Brack(
            atom(Atom::LParen),
            Box::new(Mixfix::Arg(2)),
            atom(Atom::RParen),
        )),
    );
    let right = Mixfix::Infix(
        Box::new(Mixfix::Arg("left")),
        atom(Atom::Arrow),
        Box::new(Mixfix::Brack(
            atom(Atom::LParen),
            Box::new(Mixfix::Arg("right")),
            atom(Atom::RParen),
        )),
    );

    assert!(left.eq_shape(&right));
    assert!(!left.eq_shape(&Mixfix::<&str>::Seq(vec![Mixfix::Arg("left")])));
}
