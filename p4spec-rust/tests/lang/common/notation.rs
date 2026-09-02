use p4spec_rust::{
    lang::common::{
        notation::{atom::Atom, mixfix::Mixfix},
        source::Span,
    },
    phrase,
};

fn atom(node: Atom) -> p4spec_rust::lang::common::notation::mixfix::AtomPhrase {
    phrase!(node: node, span: Span::default())
}

#[test]
fn test_into_split_preserves_nested_shape_and_argument_order() {
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

    let (mixop, args) = mixfix.into_split();

    assert_eq!(args, vec![1, 2, 3, 4]);
    assert_eq!(mixop.arity(), args.len());
    assert!(matches!(mixop, Mixfix::Seq(_)));
}
