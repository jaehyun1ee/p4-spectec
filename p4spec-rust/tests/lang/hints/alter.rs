use super::*;

#[test]
fn test_alter_validates_sequential_and_numbered_holes() {
    let hint = AlterationHint::Seq(vec![
        AlterationHint::Hole(AlterHole::Next),
        AlterationHint::Brack(
            atom("L"),
            Box::new(AlterationHint::Fuse(
                Box::new(AlterationHint::Hole(AlterHole::Num(2))),
                Box::new(AlterationHint::Hole(AlterHole::Next)),
            )),
            atom("R"),
        ),
    ]);

    assert_eq!(alter_impl::validate(&hint, &["a", "b", "c"]), Ok(()));
    assert_eq!(
        alter_impl::validate(&hint, &["a"]),
        Err(AlterationError::IndexOutOfBounds { index: 2, item_count: 1 })
    );
    assert_eq!(
        alter_impl::validate(
            &AlterationHint::Seq(vec![
                AlterationHint::Hole(AlterHole::Next),
                AlterationHint::Hole(AlterHole::Next),
            ]),
            &["a"],
        ),
        Err(AlterationError::IndexOutOfBounds { index: 1, item_count: 1 })
    );
}

#[test]
fn test_alter_realigns_outputs_around_noncontiguous_inputs() {
    let hint = AlterationHint::Brack(
        atom("L"),
        Box::new(AlterationHint::Fuse(
            Box::new(AlterationHint::Hole(AlterHole::Num(3))),
            Box::new(AlterationHint::Seq(vec![
                AlterationHint::Hole(AlterHole::Num(1)),
                AlterationHint::Hole(AlterHole::Num(3)),
            ])),
        )),
        atom("R"),
    );

    assert_eq!(
        alter_impl::realign(&hint, &InputHint::new(vec![0, 2])),
        AlterationHint::Brack(
            atom("L"),
            Box::new(AlterationHint::Fuse(
                Box::new(AlterationHint::Hole(AlterHole::Num(1))),
                Box::new(AlterationHint::Seq(vec![
                    AlterationHint::Hole(AlterHole::Num(0)),
                    AlterationHint::Hole(AlterHole::Num(1)),
                ])),
            )),
            atom("R"),
        )
    );
}

#[test]
fn test_alter_alternates_with_omission_defaults_fuse_brackets_and_other() {
    let hint = AlterationHint::Seq(vec![
        AlterationHint::Text("omit".into()),
        AlterationHint::Brack(
            atom("L"),
            Box::new(AlterationHint::Hole(AlterHole::Next)),
            atom("R"),
        ),
        AlterationHint::Fuse(
            Box::new(AlterationHint::Hole(AlterHole::Num(1))),
            Box::new(AlterationHint::Other(exp(ExpKind::Text("other".into())))),
        ),
    ]);
    let result = alter_impl::alternate(
        &hint,
        &["zero", "one"],
        &StringRenderer { empty: "_", separator: " ", fuse: "#" },
    )
    .unwrap();
    assert_eq!(result, "_ L zero R one#\"other\"");
    assert_eq!(
        alter_impl::alternate(
            &AlterationHint::Hole(AlterHole::Num(2)),
            &["zero"],
            &StringRenderer { empty: "", separator: "", fuse: "" }
        )
        .unwrap_err(),
        AlterationError::IndexOutOfBounds { index: 2, item_count: 1 }
    );
}
#[test]
fn test_alter_edge_cases_cover_init_omission_duplicates_and_next_cursor() {
    assert!(matches!(
        alter_impl::init(&exp(ExpKind::Atom(atom("A")))),
        Some(AlterationHint::Atom(_))
    ));
    assert_eq!(
        alter_impl::init(&exp(ExpKind::Seq(Vec::new()))),
        Some(AlterationHint::Seq(Vec::new()))
    );
    let nested = exp(ExpKind::Seq(vec![exp(ExpKind::Brack(
        atom("L"),
        Box::new(exp(ExpKind::Hole(Hole::Rest))),
        atom("R"),
    ))]));
    assert_eq!(
        alter_impl::init(&nested),
        Some(AlterationHint::Seq(vec![AlterationHint::Brack(
            atom("L"),
            Box::new(AlterationHint::Other(exp(ExpKind::Hole(Hole::Rest)))),
            atom("R"),
        )]))
    );
    let omitted =
        AlterationHint::Brack(atom("L"), Box::new(AlterationHint::Text("omit".into())), atom("R"));
    let rendered = alter_impl::alternate(
        &omitted,
        &[] as &[&str],
        &StringRenderer { empty: "EMPTY", separator: "|", fuse: "" },
    )
    .unwrap();
    assert_eq!(rendered, "L|R");
    let nexts = AlterationHint::Seq(vec![
        AlterationHint::Hole(AlterHole::Next),
        AlterationHint::Hole(AlterHole::Next),
    ]);

    assert_eq!(
        alter_impl::alternate(
            &nexts,
            &["a"],
            &StringRenderer { empty: "", separator: "", fuse: "" }
        )
        .unwrap_err(),
        AlterationError::IndexOutOfBounds { index: 1, item_count: 1 }
    );
}
