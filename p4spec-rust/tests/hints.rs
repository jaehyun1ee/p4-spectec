use p4spec_rust::{
    domain::{
        atom::Atom,
        source::{Position, Span, Spanned},
    },
    lang::{
        el::ast::{self, ExpKind, Hole},
        hints::{
            alter::{self, AlterationError, AlterationHint, Hole as AlterHole, Renderer},
            fields::{self, FieldError, FieldHint},
            flag, hint,
            input::{self, InputError, InputHint},
        },
    },
};

fn span(s: &str) -> Span {
    Span::new(Position::new(s, 0, 0), Position::new(s, 0, 0))
}
fn atom(s: &str) -> ast::Atom {
    Spanned::new(Atom::Keyword(s.into()), span(s))
}
fn exp(node: ExpKind) -> ast::Exp {
    Spanned::new(node, span("exp"))
}
fn id(name: &str, source: &str) -> ast::Id {
    Spanned::new(name.to_owned(), span(source))
}

struct StringRenderer {
    empty: &'static str,
    separator: &'static str,
    fuse: &'static str,
}

impl Renderer<&str> for StringRenderer {
    type Output = String;
    fn empty(&self) -> String {
        self.empty.into()
    }
    fn text(&self, text: &str) -> Option<String> {
        (text != "omit").then(|| text.into())
    }
    fn atom(&self, atom: &ast::Atom) -> String {
        atom.node.render()
    }
    fn join(&self, items: Vec<String>) -> String {
        items.join(self.separator)
    }
    fn fuse(&self, left: String, right: String) -> String {
        format!("{left}{}{right}", self.fuse)
    }
    fn other(&self, exp: &ast::Exp) -> String {
        hint::to_string(exp)
    }
    fn item(&self, item: &&str) -> String {
        (*item).into()
    }
}

#[test]
fn input_hints_validate_and_preserve_split_order() {
    let sequence = exp(ExpKind::SeqE(vec![
        exp(ExpKind::HoleE(Hole::Num(2))),
        exp(ExpKind::HoleE(Hole::Num(0))),
    ]));
    assert_eq!(input::init(&sequence), Some(InputHint::new(vec![2, 0])));
    assert_eq!(
        input::validate(&InputHint::new(vec![]), 3),
        Err(InputError::Empty)
    );
    assert_eq!(
        input::validate(&InputHint::new(vec![1, 1]), 3),
        Err(InputError::DuplicateIndex(1))
    );
    assert_eq!(
        input::validate(&InputHint::new(vec![-1]), 3),
        Err(InputError::IndexOutOfBounds {
            index: -1,
            arity: 3,
        })
    );
    assert_eq!(
        input::validate(&InputHint::new(vec![3]), 3),
        Err(InputError::IndexOutOfBounds { index: 3, arity: 3 })
    );
    let hint = InputHint::new(vec![2, 0]);
    assert_eq!(input::validate(&hint, 3), Ok(()));

    let items = ["zero", "one", "two", "three"];
    let (items_input, items_output) = input::split(&hint, &items).unwrap();
    assert_eq!(items_input, vec!["zero", "two"]);
    assert_eq!(items_output, vec!["one", "three"]);
    assert_eq!(
        input::combine(&hint, items_input, items_output),
        Ok(items.to_vec())
    );
    assert_eq!(
        input::combine(&hint, vec!["zero"], vec!["one", "three"]),
        Err(InputError::InputCountMismatch {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        input::split(&InputHint::new(vec![4]), &items),
        Err(InputError::IndexOutOfBounds { index: 4, arity: 4 })
    );
    assert_eq!(
        input::is_conditional(&InputHint::new(vec![0, 1]), &["left", "right"]),
        Ok(true)
    );
    assert_eq!(
        input::is_conditional(&InputHint::new(vec![0]), &["left", "right"]),
        Ok(false)
    );
}

#[test]
fn fields_hints_require_text_and_exact_arity() {
    let single = exp(ExpKind::TextE("left".to_owned()));
    let sequence = exp(ExpKind::SeqE(vec![
        exp(ExpKind::TextE("left".to_owned())),
        exp(ExpKind::TextE("right".to_owned())),
    ]));

    assert_eq!(
        fields::init(&single),
        Some(FieldHint::new(vec!["left".to_owned()]))
    );
    assert_eq!(
        fields::init(&sequence),
        Some(FieldHint::new(vec!["left".to_owned(), "right".to_owned()]))
    );
    assert_eq!(fields::init(&exp(ExpKind::HoleE(Hole::Next))), None);
    let fields = FieldHint::new(vec!["left".to_owned()]);
    assert_eq!(fields::validate(&fields, 1), Ok(()));
    assert_eq!(
        fields::validate(&fields, 2),
        Err(FieldError::ArityMismatch {
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn flag_hints_match_only_the_requested_identifier() {
    let hints = vec![ast::Hint {
        hintid: id("enabled", "enabled-hint"),
        hintexp: exp(ExpKind::EpsE),
    }];

    assert!(flag::init(&hints, "enabled"));
    assert!(!flag::init(&hints, "disabled"));
}

#[test]
fn hint_modules_format_exactly() {
    assert_eq!(
        input::to_string(&InputHint::new(vec![2, 0])),
        "hint(input %2 %0)"
    );
    assert!(input::eq(
        &InputHint::new(vec![2, 0]),
        &InputHint::new(vec![2, 0])
    ));
    assert!(!input::eq(
        &InputHint::new(vec![2]),
        &InputHint::new(vec![0])
    ));
    assert_eq!(
        fields::to_string(&FieldHint::new(vec!["left".into(), "right".into()])),
        "hint(fields left right)"
    );
    assert_eq!(flag::to_string(true), "hint(flag)");
    assert_eq!(flag::to_string(false), "");
    assert_eq!(hint::to_string(&exp(ExpKind::HoleE(Hole::Next))), "%");
}

#[test]
fn alter_models_validates_collects_and_realigns() {
    let hint = AlterationHint::SeqH(vec![
        AlterationHint::TextH("x".into()),
        AlterationHint::HoleH(AlterHole::Next),
        AlterationHint::BrackH(
            atom("L"),
            Box::new(AlterationHint::FuseH(
                Box::new(AlterationHint::HoleH(AlterHole::Num(3))),
                Box::new(AlterationHint::HoleH(AlterHole::Num(1))),
            )),
            atom("R"),
        ),
    ]);
    assert_eq!(alter::to_string(&hint), "hint(alter x % L %3#%1 R)");
    assert_eq!(alter::validate(&hint, &["a", "b", "c", "d"]), Ok(()));
    assert_eq!(
        alter::validate(&AlterationHint::HoleH(AlterHole::Num(4)), &["a"]),
        Err(AlterationError::IndexOutOfBounds {
            index: 4,
            item_count: 1,
        })
    );
    assert_eq!(alter::collect(&hint), vec![1, 3]);
    assert_eq!(
        alter::to_string(&alter::realign(&hint, &InputHint::new(vec![0, 2])).unwrap()),
        "hint(alter x % L %1#%0 R)"
    );
}

#[test]
fn alter_alternates_with_omission_defaults_fuse_brackets_and_other() {
    let hint = AlterationHint::SeqH(vec![
        AlterationHint::TextH("omit".into()),
        AlterationHint::BrackH(
            atom("L"),
            Box::new(AlterationHint::HoleH(AlterHole::Next)),
            atom("R"),
        ),
        AlterationHint::FuseH(
            Box::new(AlterationHint::HoleH(AlterHole::Num(1))),
            Box::new(AlterationHint::OtherH(exp(ExpKind::TextE("other".into())))),
        ),
    ]);
    let result = alter::alternate(
        &hint,
        &["zero", "one"],
        &StringRenderer {
            empty: "_",
            separator: " ",
            fuse: "#",
        },
    )
    .unwrap();
    assert_eq!(result, "_ L zero R one#\"other\"");
    assert_eq!(
        alter::alternate(
            &AlterationHint::HoleH(AlterHole::Num(2)),
            &["zero"],
            &StringRenderer {
                empty: "",
                separator: "",
                fuse: ""
            }
        )
        .unwrap_err(),
        AlterationError::IndexOutOfBounds {
            index: 2,
            item_count: 1,
        }
    );
}

#[test]
fn alter_edge_cases_cover_init_omission_duplicates_and_next_cursor() {
    assert!(matches!(
        alter::init(&exp(ExpKind::AtomE(atom("A")))),
        Some(AlterationHint::AtomH(_))
    ));
    assert_eq!(
        alter::init(&exp(ExpKind::SeqE(Vec::new()))),
        Some(AlterationHint::SeqH(Vec::new()))
    );
    let nested = exp(ExpKind::SeqE(vec![exp(ExpKind::BrackE(
        atom("L"),
        Box::new(exp(ExpKind::HoleE(Hole::Rest))),
        atom("R"),
    ))]));
    assert_eq!(
        alter::init(&nested),
        Some(AlterationHint::SeqH(vec![AlterationHint::BrackH(
            atom("L"),
            Box::new(AlterationHint::OtherH(exp(ExpKind::HoleE(Hole::Rest)))),
            atom("R"),
        )]))
    );
    let duplicate = AlterationHint::SeqH(vec![
        AlterationHint::HoleH(AlterHole::Num(2)),
        AlterationHint::HoleH(AlterHole::Num(2)),
    ]);
    assert_eq!(alter::collect(&duplicate), vec![2, 2]);
    assert_eq!(
        alter::to_string(&alter::realign(&duplicate, &InputHint::new(vec![0])).unwrap()),
        "hint(alter %0 %0)"
    );
    let omitted = AlterationHint::BrackH(
        atom("L"),
        Box::new(AlterationHint::TextH("omit".into())),
        atom("R"),
    );
    let rendered = alter::alternate(
        &omitted,
        &[] as &[&str],
        &StringRenderer {
            empty: "EMPTY",
            separator: "|",
            fuse: "",
        },
    )
    .unwrap();
    assert_eq!(rendered, "L|R");
    let nexts = AlterationHint::SeqH(vec![
        AlterationHint::HoleH(AlterHole::Next),
        AlterationHint::HoleH(AlterHole::Next),
    ]);
    assert_eq!(
        alter::validate(&nexts, &["a"]),
        Err(AlterationError::IndexOutOfBounds {
            index: 1,
            item_count: 1,
        })
    );
    assert_eq!(
        alter::alternate(
            &nexts,
            &["a"],
            &StringRenderer {
                empty: "",
                separator: "",
                fuse: ""
            }
        )
        .unwrap_err(),
        AlterationError::IndexOutOfBounds {
            index: 1,
            item_count: 1,
        }
    );
}
