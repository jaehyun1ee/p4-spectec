use p4spec_rust::domain::{
    atom::Atom,
    mixfix::{ArityMismatch, Mixfix},
    mixop,
    source::{Position, Region, Spanned, phrase_list_region},
};

fn phrase(atom: Atom) -> Spanned<Atom> {
    Spanned::new(atom, Region::none())
}

#[test]
fn source_positions_and_regions_match_ocaml_rendering() {
    let left = Position::new("spec/example.watsup", 3, 4);
    let right = Position::new("spec/example.watsup", 3, 9);
    let region = Region::new(left.clone(), right.clone());

    assert_eq!(left.to_string(), "3.5");
    assert_eq!(Position::new("", -1, 42).to_string(), "0x2a");
    assert_eq!(region.to_string(), "spec/example.watsup:3.5-3.10");
    assert_eq!(region.before(), Region::new(left.clone(), left));
    assert_eq!(region.after(), Region::new(right.clone(), right));
}

#[test]
fn region_over_uses_the_outermost_positions() {
    let first = Region::new(Position::new("a", 2, 3), Position::new("a", 2, 8));
    let second = Region::new(Position::new("a", 1, 4), Position::new("a", 4, 1));

    assert_eq!(Region::over(&[]), Region::none());
    assert_eq!(
        Region::over(&[first, second]),
        Region::new(Position::new("a", 1, 4), Position::new("a", 4, 1))
    );
}

#[test]
fn phrase_list_region_spans_first_to_last_phrase() {
    let first = Spanned::new(
        "first",
        Region::new(Position::new("a", 2, 3), Position::new("a", 2, 8)),
    );
    let last = Spanned::new(
        "last",
        Region::new(Position::new("a", 5, 1), Position::new("a", 5, 4)),
    );

    assert_eq!(phrase_list_region::<Spanned<&str>>(&[]), Region::none());
    assert_eq!(phrase_list_region(std::slice::from_ref(&first)), first.span);
    assert_eq!(
        phrase_list_region(&[first, last]),
        Region::new(Position::new("a", 2, 3), Position::new("a", 5, 4))
    );
}

#[test]
fn atoms_round_trip_the_source_spelling() {
    let spellings = [
        "WORD", "_TAG", "'+'", "<:", ":>", "|-", "-|", "->", "->_", "=>_", "==>", "~>", "~>*", ".",
        "..", "...", ";", ":", ":=", "~~", "\\", "`<", "`>", "`(", "`)", "`[", "`]", "`{", "`}",
    ];

    for spelling in spellings {
        assert_eq!(Atom::from_source(spelling).source_string(), spelling);
    }
    assert_eq!(Atom::Tag("EMPTY".into()).render(), "/* empty */");
    assert_eq!(Atom::LAngle.render(), "<");
}

#[test]
fn atom_constructors_validate_ocaml_invariants() {
    assert_eq!(Atom::tag("VALID_ID").unwrap(), Atom::Tag("VALID_ID".into()));
    assert!(Atom::tag("not-upid").is_err());
    assert_eq!(Atom::operator("++").unwrap(), Atom::Operator("++".into()));
    assert!(Atom::operator("bad'operator").is_err());
    assert!(Atom::operator("bad\noperator").is_err());
}

#[test]
fn mixfix_fill_split_and_render_match_ocaml_order() {
    let mixop = Mixfix::Seq(vec![
        Mixfix::Atom(phrase(Atom::Keyword("IF".into()))),
        Mixfix::Arg(()),
        Mixfix::Atom(phrase(Atom::Keyword("THEN".into()))),
        Mixfix::Brack(
            phrase(Atom::LParen),
            Box::new(Mixfix::Arg(())),
            phrase(Atom::RParen),
        ),
    ]);

    assert_eq!(mixop.arity(), 2);
    assert_eq!(mixop.to_string(), "`IF % THEN (%)`");

    let filled = Mixfix::fill(&mixop, ["condition", "body"]).unwrap();
    assert_eq!(filled.args(), vec![&"condition", &"body"]);
    assert_eq!(
        filled.render(|atom| atom.node.render(), |arg| (*arg).to_owned()),
        "IF condition THEN ( body )"
    );

    let (split_mixop, args) = filled.split();
    assert_eq!(split_mixop, mixop);
    assert_eq!(args, vec![&"condition", &"body"]);
}

#[test]
fn mixfix_reports_both_arity_mismatches() {
    let mixop = Mixfix::Seq(vec![Mixfix::Arg(()), Mixfix::Arg(())]);

    assert_eq!(
        Mixfix::fill(&mixop, [1]).unwrap_err(),
        ArityMismatch::TooFew
    );
    assert_eq!(
        Mixfix::fill(&mixop, [1, 2, 3]).unwrap_err(),
        ArityMismatch::TooMany
    );
}

#[test]
fn mixfix_equality_ignores_atom_source_regions() {
    let atom = Atom::Keyword("SAME".into());
    let left = Mixfix::<()>::Atom(Spanned::new(atom.clone(), Region::for_file("left.watsup")));
    let right = Mixfix::<()>::Atom(Spanned::new(atom, Region::for_file("right.watsup")));

    assert_eq!(left, right);
}

#[test]
fn mixfix_shape_comparison_ignores_argument_values() {
    let left = Mixfix::Seq(vec![
        Mixfix::Atom(phrase(Atom::Keyword("X".into()))),
        Mixfix::Arg(1),
    ]);
    let right = Mixfix::Seq(vec![
        Mixfix::Atom(phrase(Atom::Keyword("X".into()))),
        Mixfix::Arg("different"),
    ]);

    assert!(left.same_shape(&right));
    assert_eq!(left.cmp_shape(&right), std::cmp::Ordering::Equal);
}

#[test]
fn mixfix_fold_map_atoms_and_assemble_follow_source_order() {
    let mixfix = Mixfix::Seq(vec![
        Mixfix::Atom(phrase(Atom::Keyword("DROP".into()))),
        Mixfix::Arg("left"),
        Mixfix::Brack(
            phrase(Atom::LParen),
            Box::new(Mixfix::Arg("right")),
            phrase(Atom::RParen),
        ),
    ]);

    assert_eq!(
        mixfix.fold(Vec::new(), |mut args, arg| {
            args.push(*arg);
            args
        }),
        vec!["left", "right"]
    );

    let renamed = mixfix.map_atoms(|atom| {
        if atom.node == Atom::Keyword("DROP".into()) {
            phrase(Atom::Keyword("KEEP".into()))
        } else {
            atom.clone()
        }
    });
    assert_eq!(renamed.to_string(), "`KEEP % (%)`");

    let mut visited_args = Vec::new();
    renamed.iter(|arg| visited_args.push(*arg));
    assert_eq!(visited_args, vec!["left", "right"]);

    let mut visited_atoms = Vec::new();
    renamed.iter_atoms(|atom| visited_atoms.push(atom.node.clone()));
    assert_eq!(
        visited_atoms,
        vec![Atom::Keyword("KEEP".into()), Atom::LParen, Atom::RParen]
    );

    let pieces = renamed.map(|arg| (*arg).to_owned());
    let assembled = pieces.assemble(
        String::new(),
        " ".to_owned(),
        |atom| match atom.node {
            Atom::Keyword(_) => None,
            _ => Some(atom.node.render()),
        },
        |left, right| left + &right,
    );
    assert_eq!(assembled, "left ( right )");
}

#[test]
fn mixfix_render_preserves_empty_arguments_but_omits_empty_atoms() {
    let mixfix = Mixfix::Seq(vec![
        Mixfix::Arg(""),
        Mixfix::Atom(phrase(Atom::Keyword("VISIBLE".into()))),
        Mixfix::Atom(phrase(Atom::Tag("HIDDEN".into()))),
    ]);

    assert_eq!(
        mixfix.render(
            |atom| match atom.node {
                Atom::Tag(_) => String::new(),
                _ => atom.node.render(),
            },
            |arg| (*arg).to_owned(),
        ),
        " VISIBLE"
    );
}

#[test]
fn mixop_facade_fills_and_renders_arguments() {
    let operator = Mixfix::Seq(vec![
        Mixfix::Atom(phrase(Atom::Keyword("CALL".into()))),
        Mixfix::Arg(()),
    ]);

    assert_eq!(mixop::arity(&operator), 1);
    assert_eq!(mixop::string_of_mixop(&operator), "`CALL %`");
    assert_eq!(
        mixop::assemble(&operator, ["argument".to_owned()], |atom| atom
            .node
            .render())
        .unwrap(),
        "CALL argument"
    );
}

#[test]
fn atom_matrix_splits_at_each_argument() {
    let mixfix = Mixfix::Seq(vec![
        Mixfix::Atom(phrase(Atom::Keyword("IF".into()))),
        Mixfix::Arg(1),
        Mixfix::Atom(phrase(Atom::Keyword("THEN".into()))),
        Mixfix::Arg(2),
    ]);

    let matrix: Vec<Vec<&Atom>> = mixfix
        .atoms_matrix()
        .into_iter()
        .map(|row| row.into_iter().map(|atom| &atom.node).collect())
        .collect();
    assert_eq!(
        matrix,
        vec![
            vec![&Atom::Keyword("IF".into())],
            vec![&Atom::Keyword("THEN".into())],
            vec![],
        ]
    );
}
