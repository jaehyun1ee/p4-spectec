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
    assert!(!left.eq_shape(&Mixfix::Seq(vec![Mixfix::Arg("left")])));
}
