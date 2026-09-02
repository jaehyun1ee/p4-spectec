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

#[test]
fn test_same_shape_borrows_nested_mixfixes_and_ignores_arguments() {
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

    assert!(left.same_shape(&right));
    assert!(!left.same_shape(&Mixfix::Seq(vec![Mixfix::Arg("left")])));
}

#[test]
fn test_try_eq_by_compares_nested_arguments_without_collecting_them() {
    let left = Mixfix::Seq(vec![Mixfix::Arg(1), Mixfix::Arg(2)]);
    let right = Mixfix::Seq(vec![Mixfix::Arg(1), Mixfix::Arg(2)]);
    let mut compared = Vec::new();

    let equal: Result<bool, ()> = left.try_eq_by(&right, |left, right| {
        compared.push((*left, *right));
        Ok(left == right)
    });

    assert_eq!(equal, Ok(true));
    assert_eq!(compared, vec![(1, 1), (2, 2)]);
}
