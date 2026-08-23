use p4spec_rust::{
    domain::{
        atom::Atom,
        source::{Region, Spanned},
    },
    lang::{
        el::ast::{self, ExpKind, Hole},
        hints::{
            alter::{self, Hole as AlterHole, T},
            fields, flag, hint, input,
        },
    },
};

fn span(s: &str) -> Region {
    Region::for_file(s)
}
fn atom(s: &str) -> ast::Atom {
    Spanned::new(Atom::Keyword(s.into()), span(s))
}
fn exp(node: ExpKind) -> ast::Exp {
    Spanned::new(node, span("exp"))
}

#[test]
fn hint_modules_format_exactly() {
    assert_eq!(input::to_string(&[2, 0]), "hint(input %2 %0)");
    assert!(input::eq(&[2, 0], &[2, 0]));
    assert!(!input::eq(&[2], &[0]));
    assert_eq!(
        fields::to_string(&["left".into(), "right".into()]),
        "hint(fields left right)"
    );
    assert_eq!(flag::to_string(true), "hint(flag)");
    assert_eq!(flag::to_string(false), "");
    assert_eq!(hint::to_string(&exp(ExpKind::HoleE(Hole::Next))), "%");
}

#[test]
fn alter_models_validates_collects_and_realigns() {
    let hint = T::SeqH(vec![
        T::TextH("x".into()),
        T::HoleH(AlterHole::Next),
        T::BrackH(
            atom("L"),
            Box::new(T::FuseH(
                Box::new(T::HoleH(AlterHole::Num(3))),
                Box::new(T::HoleH(AlterHole::Num(1))),
            )),
            atom("R"),
        ),
    ]);
    assert_eq!(alter::to_string(&hint), "hint(alter x % L %3#%1 R)");
    assert_eq!(alter::validate(&hint, &["a", "b", "c", "d"]), Ok(()));
    assert_eq!(
        alter::validate(&T::HoleH(AlterHole::Num(4)), &["a"]),
        Err("index 4 out of bounds".into())
    );
    assert_eq!(alter::collect(&hint), vec![1, 3]);
    assert_eq!(
        alter::to_string(&alter::realign(&hint, &vec![0, 2]).unwrap()),
        "hint(alter x % L %1#%0 R)"
    );
}

#[test]
fn alter_alternates_with_omission_defaults_fuse_brackets_and_other() {
    let hint = T::SeqH(vec![
        T::TextH("omit".into()),
        T::BrackH(atom("L"), Box::new(T::HoleH(AlterHole::Next)), atom("R")),
        T::FuseH(
            Box::new(T::HoleH(AlterHole::Num(1))),
            Box::new(T::OtherH(exp(ExpKind::TextE("other".into())))),
        ),
    ]);
    let result = alter::alternate(
        &hint,
        &["zero", "one"],
        "_".to_owned(),
        |text| (text != "omit").then(|| text.to_owned()),
        |atom| atom.node.render(),
        |items| items.join(" "),
        |left, right| format!("{left}#{right}"),
        hint::to_string,
        |item| item.to_string(),
    )
    .unwrap();
    assert_eq!(result, "_ L zero R one#\"other\"");
    assert!(
        alter::alternate(
            &T::HoleH(AlterHole::Num(2)),
            &["zero"],
            String::new(),
            |_| None,
            |_| String::new(),
            |_| String::new(),
            |_, _| String::new(),
            |_| String::new(),
            |item| item.to_string()
        )
        .is_err()
    );
}

#[test]
fn alter_edge_cases_cover_init_omission_duplicates_and_next_cursor() {
    assert!(matches!(
        alter::init(&exp(ExpKind::AtomE(atom("A")))),
        Some(T::AtomH(_))
    ));
    assert_eq!(
        alter::init(&exp(ExpKind::SeqE(Vec::new()))),
        Some(T::SeqH(Vec::new()))
    );
    let nested = exp(ExpKind::SeqE(vec![exp(ExpKind::BrackE(
        atom("L"),
        Box::new(exp(ExpKind::HoleE(Hole::Rest))),
        atom("R"),
    ))]));
    assert_eq!(
        alter::init(&nested),
        Some(T::SeqH(vec![T::BrackH(
            atom("L"),
            Box::new(T::OtherH(exp(ExpKind::HoleE(Hole::Rest)))),
            atom("R"),
        )]))
    );
    let duplicate = T::SeqH(vec![
        T::HoleH(AlterHole::Num(2)),
        T::HoleH(AlterHole::Num(2)),
    ]);
    assert_eq!(alter::collect(&duplicate), vec![2, 2]);
    assert_eq!(
        alter::to_string(&alter::realign(&duplicate, &vec![0]).unwrap()),
        "hint(alter %0 %0)"
    );
    let omitted = T::BrackH(atom("L"), Box::new(T::TextH("omit".into())), atom("R"));
    let rendered = alter::alternate(
        &omitted,
        &[] as &[&str],
        "EMPTY".into(),
        |_| None,
        |a| a.node.render(),
        |parts| parts.join("|"),
        |a, b| format!("{a}{b}"),
        |_| String::new(),
        |_| String::new(),
    )
    .unwrap();
    assert_eq!(rendered, "L|R");
    let nexts = T::SeqH(vec![T::HoleH(AlterHole::Next), T::HoleH(AlterHole::Next)]);
    assert!(
        alter::alternate(
            &nexts,
            &["a"],
            String::new(),
            |_| None,
            |_| String::new(),
            |x| x.join(""),
            |a, b| format!("{a}{b}"),
            |_| String::new(),
            |x| x.to_string()
        )
        .is_err()
    );
}
