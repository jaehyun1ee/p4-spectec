use std::str::FromStr;

use num_bigint::BigInt;
use p4spec_rust::{
    domain::{
        atom::Atom as DomainAtom,
        external_data::ExternalData,
        mixfix::{Mixfix, Mixop},
        source::{Info, Phrase, Region},
    },
    lang::{
        el,
        hints::input::T as InputHint,
        il, sl,
        xl::{r#bool as xl_bool, num},
    },
};

fn phrase<T>(it: T) -> Phrase<T> {
    Phrase::new(it, Region::for_file("spec/test.watsup"))
}

fn domain_atom() -> DomainAtom {
    DomainAtom::Keyword("ATOM".into())
}

fn mixop() -> Mixop {
    Mixfix::Seq(vec![Mixfix::Atom(phrase(domain_atom())), Mixfix::Arg(())])
}

#[test]
fn xl_and_external_data_represent_every_canonical_data_variant() {
    let large = BigInt::from_str("1234567890123456789012345678901234567890")
        .expect("hand-written arbitrary-precision integer");
    let numbers = [num::T::Nat(large.clone()), num::T::Int(-large.clone())];
    assert_eq!(
        numbers.map(|number| match number {
            num::T::Nat(_) => "Nat",
            num::T::Int(_) => "Int",
        }),
        ["Nat", "Int"]
    );

    assert_eq!(
        [num::Typ::NatT, num::Typ::IntT].map(|typ| match typ {
            num::Typ::NatT => "NatT",
            num::Typ::IntT => "IntT",
        }),
        ["NatT", "IntT"]
    );
    assert_eq!(
        [num::UnOp::PlusOp, num::UnOp::MinusOp].map(|operator| match operator {
            num::UnOp::PlusOp => "PlusOp",
            num::UnOp::MinusOp => "MinusOp",
        }),
        ["PlusOp", "MinusOp"]
    );
    assert_eq!(
        [
            num::BinOp::AddOp,
            num::BinOp::SubOp,
            num::BinOp::MulOp,
            num::BinOp::DivOp,
            num::BinOp::ModOp,
            num::BinOp::PowOp,
        ]
        .map(|operator| match operator {
            num::BinOp::AddOp => "AddOp",
            num::BinOp::SubOp => "SubOp",
            num::BinOp::MulOp => "MulOp",
            num::BinOp::DivOp => "DivOp",
            num::BinOp::ModOp => "ModOp",
            num::BinOp::PowOp => "PowOp",
        }),
        ["AddOp", "SubOp", "MulOp", "DivOp", "ModOp", "PowOp"]
    );
    assert_eq!(
        [
            num::CmpOp::LtOp,
            num::CmpOp::GtOp,
            num::CmpOp::LeOp,
            num::CmpOp::GeOp,
        ]
        .map(|operator| match operator {
            num::CmpOp::LtOp => "LtOp",
            num::CmpOp::GtOp => "GtOp",
            num::CmpOp::LeOp => "LeOp",
            num::CmpOp::GeOp => "GeOp",
        }),
        ["LtOp", "GtOp", "LeOp", "GeOp"]
    );

    assert!(matches!(xl_bool::T::BoolT, xl_bool::T::BoolT));
    assert!(matches!(xl_bool::Typ::BoolT, xl_bool::Typ::BoolT));
    assert!(matches!(xl_bool::UnOp::NotOp, xl_bool::UnOp::NotOp));
    assert_eq!(
        [
            xl_bool::BinOp::AndOp,
            xl_bool::BinOp::OrOp,
            xl_bool::BinOp::ImplOp,
            xl_bool::BinOp::EquivOp,
        ]
        .map(|operator| match operator {
            xl_bool::BinOp::AndOp => "AndOp",
            xl_bool::BinOp::OrOp => "OrOp",
            xl_bool::BinOp::ImplOp => "ImplOp",
            xl_bool::BinOp::EquivOp => "EquivOp",
        }),
        ["AndOp", "OrOp", "ImplOp", "EquivOp"]
    );
    assert_eq!(
        [xl_bool::CmpOp::EqOp, xl_bool::CmpOp::NeOp].map(|operator| match operator {
            xl_bool::CmpOp::EqOp => "EqOp",
            xl_bool::CmpOp::NeOp => "NeOp",
        }),
        ["EqOp", "NeOp"]
    );

    let external_data = vec![
        ExternalData::Null,
        ExternalData::Bool(true),
        ExternalData::Int(7),
        ExternalData::Intlit("12345678901234567890".into()),
        ExternalData::Float(3.5),
        ExternalData::String("text".into()),
        ExternalData::Assoc(vec![("key".into(), ExternalData::Null)]),
        ExternalData::List(vec![ExternalData::Bool(false)]),
        ExternalData::Tuple(vec![ExternalData::Int(1)]),
        ExternalData::Variant(
            "Some".into(),
            Some(Box::new(ExternalData::String("payload".into()))),
        ),
    ];
    let tags: Vec<_> = external_data
        .iter()
        .map(|data| match data {
            ExternalData::Null => "Null",
            ExternalData::Bool(_) => "Bool",
            ExternalData::Int(_) => "Int",
            ExternalData::Intlit(_) => "Intlit",
            ExternalData::Float(_) => "Float",
            ExternalData::String(_) => "String",
            ExternalData::Assoc(_) => "Assoc",
            ExternalData::List(_) => "List",
            ExternalData::Tuple(_) => "Tuple",
            ExternalData::Variant(_, _) => "Variant",
        })
        .collect();
    assert_eq!(
        tags,
        [
            "Null", "Bool", "Int", "Intlit", "Float", "String", "Assoc", "List", "Tuple",
            "Variant",
        ]
    );
}

fn el_id(name: &str) -> el::ast::Id {
    phrase(name.to_owned())
}

fn el_atom() -> el::ast::Atom {
    phrase(domain_atom())
}

fn el_plain(kind: el::ast::PlainTypKind) -> el::ast::PlainTyp {
    phrase(kind)
}

fn el_exp(kind: el::ast::ExpKind) -> el::ast::Exp {
    phrase(kind)
}

fn el_path(kind: el::ast::PathKind) -> el::ast::Path {
    phrase(kind)
}

fn el_arg(kind: el::ast::ArgKind) -> el::ast::Arg {
    phrase(kind)
}

fn el_num() -> el::ast::Num {
    num::T::Nat(BigInt::from(13))
}

fn el_plain_tag(typ: &el::ast::PlainTypKind) -> &'static str {
    match typ {
        el::ast::PlainTypKind::BoolT => "BoolT",
        el::ast::PlainTypKind::NumT(_) => "NumT",
        el::ast::PlainTypKind::TextT => "TextT",
        el::ast::PlainTypKind::VarT(_, _) => "VarT",
        el::ast::PlainTypKind::ParenT(_) => "ParenT",
        el::ast::PlainTypKind::TupleT(_) => "TupleT",
        el::ast::PlainTypKind::IterT(_, _) => "IterT",
    }
}

fn el_exp_tag(exp: &el::ast::ExpKind) -> &'static str {
    match exp {
        el::ast::ExpKind::BoolE(_) => "BoolE",
        el::ast::ExpKind::NumE(_, _) => "NumE",
        el::ast::ExpKind::TextE(_) => "TextE",
        el::ast::ExpKind::VarE(_) => "VarE",
        el::ast::ExpKind::UnE(_, _) => "UnE",
        el::ast::ExpKind::BinE(_, _, _) => "BinE",
        el::ast::ExpKind::CmpE(_, _, _) => "CmpE",
        el::ast::ExpKind::ArithE(_) => "ArithE",
        el::ast::ExpKind::EpsE => "EpsE",
        el::ast::ExpKind::ListE(_) => "ListE",
        el::ast::ExpKind::ConsE(_, _) => "ConsE",
        el::ast::ExpKind::CatE(_, _) => "CatE",
        el::ast::ExpKind::IdxE(_, _) => "IdxE",
        el::ast::ExpKind::SliceE(_, _, _) => "SliceE",
        el::ast::ExpKind::LenE(_) => "LenE",
        el::ast::ExpKind::MemE(_, _) => "MemE",
        el::ast::ExpKind::StrE(_) => "StrE",
        el::ast::ExpKind::DotE(_, _) => "DotE",
        el::ast::ExpKind::UpdE(_, _, _) => "UpdE",
        el::ast::ExpKind::ParenE(_) => "ParenE",
        el::ast::ExpKind::TupleE(_) => "TupleE",
        el::ast::ExpKind::CallE(_, _, _) => "CallE",
        el::ast::ExpKind::IterE(_, _) => "IterE",
        el::ast::ExpKind::SubE(_, _) => "SubE",
        el::ast::ExpKind::AtomE(_) => "AtomE",
        el::ast::ExpKind::SeqE(_) => "SeqE",
        el::ast::ExpKind::InfixE(_, _, _) => "InfixE",
        el::ast::ExpKind::BrackE(_, _, _) => "BrackE",
        el::ast::ExpKind::HoleE(_) => "HoleE",
        el::ast::ExpKind::FuseE(_, _) => "FuseE",
        el::ast::ExpKind::UnparenE(_) => "UnparenE",
        el::ast::ExpKind::LatexE(_) => "LatexE",
    }
}

#[test]
fn el_hint_closure_represents_every_required_variant() {
    assert_eq!(
        [el::ast::Iter::Opt, el::ast::Iter::List].map(|iter| match iter {
            el::ast::Iter::Opt => "Opt",
            el::ast::Iter::List => "List",
        }),
        ["Opt", "List"]
    );

    let plain_types = [
        el::ast::PlainTypKind::BoolT,
        el::ast::PlainTypKind::NumT(num::Typ::NatT),
        el::ast::PlainTypKind::TextT,
        el::ast::PlainTypKind::VarT(el_id("typ"), vec![phrase(el::ast::PlainTypKind::BoolT)]),
        el::ast::PlainTypKind::ParenT(Box::new(el_plain(el::ast::PlainTypKind::TextT))),
        el::ast::PlainTypKind::TupleT(vec![el_plain(el::ast::PlainTypKind::BoolT)]),
        el::ast::PlainTypKind::IterT(
            Box::new(el_plain(el::ast::PlainTypKind::TextT)),
            el::ast::Iter::List,
        ),
    ];
    assert_eq!(
        plain_types.iter().map(el_plain_tag).collect::<Vec<_>>(),
        [
            "BoolT", "NumT", "TextT", "VarT", "ParenT", "TupleT", "IterT"
        ]
    );

    assert_eq!(
        [el::ast::NumOp::DecOp, el::ast::NumOp::HexOp].map(|op| match op {
            el::ast::NumOp::DecOp => "DecOp",
            el::ast::NumOp::HexOp => "HexOp",
        }),
        ["DecOp", "HexOp"]
    );
    assert_eq!(
        [
            el::ast::UnOp::NotOp,
            el::ast::UnOp::PlusOp,
            el::ast::UnOp::MinusOp
        ]
        .map(|op| match op {
            el::ast::UnOp::NotOp => "NotOp",
            el::ast::UnOp::PlusOp => "PlusOp",
            el::ast::UnOp::MinusOp => "MinusOp",
        }),
        ["NotOp", "PlusOp", "MinusOp"]
    );
    assert_eq!(
        [
            el::ast::BinOp::AndOp,
            el::ast::BinOp::OrOp,
            el::ast::BinOp::ImplOp,
            el::ast::BinOp::EquivOp,
            el::ast::BinOp::AddOp,
            el::ast::BinOp::SubOp,
            el::ast::BinOp::MulOp,
            el::ast::BinOp::DivOp,
            el::ast::BinOp::ModOp,
            el::ast::BinOp::PowOp,
        ]
        .map(|op| match op {
            el::ast::BinOp::AndOp => "AndOp",
            el::ast::BinOp::OrOp => "OrOp",
            el::ast::BinOp::ImplOp => "ImplOp",
            el::ast::BinOp::EquivOp => "EquivOp",
            el::ast::BinOp::AddOp => "AddOp",
            el::ast::BinOp::SubOp => "SubOp",
            el::ast::BinOp::MulOp => "MulOp",
            el::ast::BinOp::DivOp => "DivOp",
            el::ast::BinOp::ModOp => "ModOp",
            el::ast::BinOp::PowOp => "PowOp",
        }),
        [
            "AndOp", "OrOp", "ImplOp", "EquivOp", "AddOp", "SubOp", "MulOp", "DivOp", "ModOp",
            "PowOp",
        ]
    );
    assert_eq!(
        [
            el::ast::CmpOp::EqOp,
            el::ast::CmpOp::NeOp,
            el::ast::CmpOp::LtOp,
            el::ast::CmpOp::GtOp,
            el::ast::CmpOp::LeOp,
            el::ast::CmpOp::GeOp,
        ]
        .map(|op| match op {
            el::ast::CmpOp::EqOp => "EqOp",
            el::ast::CmpOp::NeOp => "NeOp",
            el::ast::CmpOp::LtOp => "LtOp",
            el::ast::CmpOp::GtOp => "GtOp",
            el::ast::CmpOp::LeOp => "LeOp",
            el::ast::CmpOp::GeOp => "GeOp",
        }),
        ["EqOp", "NeOp", "LtOp", "GtOp", "LeOp", "GeOp"]
    );

    let path = || el_path(el::ast::PathKind::RootP);
    let exp = || el_exp(el::ast::ExpKind::EpsE);
    let expressions = vec![
        el::ast::ExpKind::BoolE(true),
        el::ast::ExpKind::NumE(el::ast::NumOp::DecOp, el_num()),
        el::ast::ExpKind::TextE("text".into()),
        el::ast::ExpKind::VarE(el_id("x")),
        el::ast::ExpKind::UnE(el::ast::UnOp::NotOp, Box::new(exp())),
        el::ast::ExpKind::BinE(Box::new(exp()), el::ast::BinOp::AddOp, Box::new(exp())),
        el::ast::ExpKind::CmpE(Box::new(exp()), el::ast::CmpOp::EqOp, Box::new(exp())),
        el::ast::ExpKind::ArithE(Box::new(exp())),
        el::ast::ExpKind::EpsE,
        el::ast::ExpKind::ListE(vec![exp()]),
        el::ast::ExpKind::ConsE(Box::new(exp()), Box::new(exp())),
        el::ast::ExpKind::CatE(Box::new(exp()), Box::new(exp())),
        el::ast::ExpKind::IdxE(Box::new(exp()), Box::new(exp())),
        el::ast::ExpKind::SliceE(Box::new(exp()), Box::new(exp()), Box::new(exp())),
        el::ast::ExpKind::LenE(Box::new(exp())),
        el::ast::ExpKind::MemE(Box::new(exp()), Box::new(exp())),
        el::ast::ExpKind::StrE(vec![(el_atom(), exp())]),
        el::ast::ExpKind::DotE(Box::new(exp()), el_atom()),
        el::ast::ExpKind::UpdE(Box::new(exp()), path(), Box::new(exp())),
        el::ast::ExpKind::ParenE(Box::new(exp())),
        el::ast::ExpKind::TupleE(vec![exp()]),
        el::ast::ExpKind::CallE(
            el_id("call"),
            vec![phrase(el::ast::PlainTypKind::BoolT)],
            vec![el_arg(el::ast::ArgKind::ExpA(exp()))],
        ),
        el::ast::ExpKind::IterE(Box::new(exp()), el::ast::Iter::Opt),
        el::ast::ExpKind::SubE(Box::new(exp()), el_plain(el::ast::PlainTypKind::BoolT)),
        el::ast::ExpKind::AtomE(el_atom()),
        el::ast::ExpKind::SeqE(vec![exp()]),
        el::ast::ExpKind::InfixE(Box::new(exp()), el_atom(), Box::new(exp())),
        el::ast::ExpKind::BrackE(el_atom(), Box::new(exp()), el_atom()),
        el::ast::ExpKind::HoleE(el::ast::Hole::Num(2)),
        el::ast::ExpKind::FuseE(Box::new(exp()), Box::new(exp())),
        el::ast::ExpKind::UnparenE(Box::new(exp())),
        el::ast::ExpKind::LatexE("x_{1}".into()),
    ];
    assert_eq!(
        expressions.iter().map(el_exp_tag).collect::<Vec<_>>(),
        [
            "BoolE", "NumE", "TextE", "VarE", "UnE", "BinE", "CmpE", "ArithE", "EpsE", "ListE",
            "ConsE", "CatE", "IdxE", "SliceE", "LenE", "MemE", "StrE", "DotE", "UpdE", "ParenE",
            "TupleE", "CallE", "IterE", "SubE", "AtomE", "SeqE", "InfixE", "BrackE", "HoleE",
            "FuseE", "UnparenE", "LatexE",
        ]
    );

    assert_eq!(
        [
            el::ast::Hole::Num(0),
            el::ast::Hole::Next,
            el::ast::Hole::Rest,
            el::ast::Hole::None,
        ]
        .map(|hole| match hole {
            el::ast::Hole::Num(_) => "Num",
            el::ast::Hole::Next => "Next",
            el::ast::Hole::Rest => "Rest",
            el::ast::Hole::None => "None",
        }),
        ["Num", "Next", "Rest", "None"]
    );

    let paths = [
        el::ast::PathKind::RootP,
        el::ast::PathKind::IdxP(Box::new(path()), Box::new(exp())),
        el::ast::PathKind::SliceP(Box::new(path()), Box::new(exp()), Box::new(exp())),
        el::ast::PathKind::DotP(Box::new(path()), el_atom()),
    ];
    assert_eq!(
        paths.map(|path| match path {
            el::ast::PathKind::RootP => "RootP",
            el::ast::PathKind::IdxP(_, _) => "IdxP",
            el::ast::PathKind::SliceP(_, _, _) => "SliceP",
            el::ast::PathKind::DotP(_, _) => "DotP",
        }),
        ["RootP", "IdxP", "SliceP", "DotP"]
    );
    assert_eq!(
        [
            el::ast::ArgKind::ExpA(exp()),
            el::ast::ArgKind::DefA(el_id("def")),
        ]
        .map(|arg| match arg {
            el::ast::ArgKind::ExpA(_) => "ExpA",
            el::ast::ArgKind::DefA(_) => "DefA",
        }),
        ["ExpA", "DefA"]
    );

    let hint = el::ast::Hint {
        hintid: el_id("format"),
        hintexp: el_exp(el::ast::ExpKind::InfixE(
            Box::new(el_exp(el::ast::ExpKind::HoleE(el::ast::Hole::Num(0)))),
            el_atom(),
            Box::new(el_exp(el::ast::ExpKind::HoleE(el::ast::Hole::Rest))),
        )),
    };
    assert_eq!(hint.hintid.it, "format");
    assert!(matches!(hint.hintexp.it, el::ast::ExpKind::InfixE(_, _, _)));
}

fn il_id(name: &str) -> il::ast::Id {
    phrase(name.to_owned())
}

fn il_atom() -> il::ast::Atom {
    phrase(domain_atom())
}

fn il_typ(kind: il::ast::TypKind) -> il::ast::Typ {
    phrase(kind)
}

fn il_exp(kind: il::ast::ExpKind) -> il::ast::Exp {
    Info::with_note(
        kind,
        Region::for_file("spec/test.watsup"),
        il::ast::TypKind::BoolT,
    )
}

fn il_path(kind: il::ast::PathKind) -> il::ast::Path {
    Info::with_note(
        kind,
        Region::for_file("spec/test.watsup"),
        il::ast::TypKind::BoolT,
    )
}

fn il_param(kind: il::ast::ParamKind) -> il::ast::Param {
    phrase(kind)
}

fn il_arg(kind: il::ast::ArgKind) -> il::ast::Arg {
    phrase(kind)
}

fn il_prem(kind: il::ast::PremKind) -> il::ast::Prem {
    phrase(kind)
}

fn il_value(kind: il::ast::ValueKind) -> il::ast::Value {
    Info::with_note(
        kind,
        Region::for_file("spec/test.watsup"),
        il::ast::VNote {
            vid: 17,
            typ: il::ast::TypKind::BoolT,
            vhash: 23,
        },
    )
}

fn il_not_exp(exp: il::ast::Exp) -> il::ast::NotExp {
    Mixfix::Arg(exp)
}

fn il_hint() -> il::ast::Hint {
    el::ast::Hint {
        hintid: el_id("hint"),
        hintexp: el_exp(el::ast::ExpKind::EpsE),
    }
}

fn il_typ_tag(typ: &il::ast::TypKind) -> &'static str {
    match typ {
        il::ast::TypKind::BoolT => "BoolT",
        il::ast::TypKind::NumT(_) => "NumT",
        il::ast::TypKind::TextT => "TextT",
        il::ast::TypKind::VarT(_, _) => "VarT",
        il::ast::TypKind::TupleT(_) => "TupleT",
        il::ast::TypKind::IterT(_, _) => "IterT",
        il::ast::TypKind::FuncT(_, _, _) => "FuncT",
    }
}

fn il_value_tag(value: &il::ast::ValueKind) -> &'static str {
    match value {
        il::ast::ValueKind::BoolV(_) => "BoolV",
        il::ast::ValueKind::NumV(_) => "NumV",
        il::ast::ValueKind::TextV(_) => "TextV",
        il::ast::ValueKind::StructV(_) => "StructV",
        il::ast::ValueKind::CaseV(_) => "CaseV",
        il::ast::ValueKind::TupleV(_) => "TupleV",
        il::ast::ValueKind::OptV(_) => "OptV",
        il::ast::ValueKind::ListV(_) => "ListV",
        il::ast::ValueKind::FuncV(_) => "FuncV",
        il::ast::ValueKind::ExternV(_) => "ExternV",
    }
}

#[test]
fn il_types_values_and_operators_represent_every_variant() {
    assert_eq!(
        [il::ast::Iter::Opt, il::ast::Iter::List].map(|iter| match iter {
            il::ast::Iter::Opt => "Opt",
            il::ast::Iter::List => "List",
        }),
        ["Opt", "List"]
    );

    let types = [
        il::ast::TypKind::BoolT,
        il::ast::TypKind::NumT(num::Typ::IntT),
        il::ast::TypKind::TextT,
        il::ast::TypKind::VarT(il_id("T"), vec![phrase(il::ast::TypKind::BoolT)]),
        il::ast::TypKind::TupleT(vec![il_typ(il::ast::TypKind::TextT)]),
        il::ast::TypKind::IterT(
            Box::new(il_typ(il::ast::TypKind::BoolT)),
            il::ast::Iter::Opt,
        ),
        il::ast::TypKind::FuncT(
            vec![phrase("X".into())],
            vec![il_typ(il::ast::TypKind::BoolT)],
            Box::new(il_typ(il::ast::TypKind::TextT)),
        ),
    ];
    assert_eq!(
        types.iter().map(il_typ_tag).collect::<Vec<_>>(),
        ["BoolT", "NumT", "TextT", "VarT", "TupleT", "IterT", "FuncT"]
    );

    let defined_types = [
        il::ast::DefTypKind::PlainT(il_typ(il::ast::TypKind::BoolT)),
        il::ast::DefTypKind::StructT(vec![(il_atom(), il_typ(il::ast::TypKind::TextT))]),
        il::ast::DefTypKind::VariantT(vec![(
            phrase(Mixfix::Arg(il_typ(il::ast::TypKind::BoolT))),
            phrase((il_id("origin"), vec![])),
            vec![il_hint()],
        )]),
    ];
    assert_eq!(
        defined_types.map(|typ| match typ {
            il::ast::DefTypKind::PlainT(_) => "PlainT",
            il::ast::DefTypKind::StructT(_) => "StructT",
            il::ast::DefTypKind::VariantT(_) => "VariantT",
        }),
        ["PlainT", "StructT", "VariantT"]
    );

    let value = || il_value(il::ast::ValueKind::BoolV(false));
    let values = vec![
        il::ast::ValueKind::BoolV(true),
        il::ast::ValueKind::NumV(num::T::Int(BigInt::from(-9))),
        il::ast::ValueKind::TextV("text".into()),
        il::ast::ValueKind::StructV(vec![(il_atom(), value())]),
        il::ast::ValueKind::CaseV(Box::new(Mixfix::Arg(value()))),
        il::ast::ValueKind::TupleV(vec![value()]),
        il::ast::ValueKind::OptV(Some(Box::new(value()))),
        il::ast::ValueKind::ListV(vec![value()]),
        il::ast::ValueKind::FuncV(il_id("function")),
        il::ast::ValueKind::ExternV(ExternalData::Variant("None".into(), None)),
    ];
    assert_eq!(
        values.iter().map(il_value_tag).collect::<Vec<_>>(),
        [
            "BoolV", "NumV", "TextV", "StructV", "CaseV", "TupleV", "OptV", "ListV", "FuncV",
            "ExternV",
        ]
    );
    let noted = il_value(il::ast::ValueKind::NumV(num::T::Nat(BigInt::from(5))));
    assert_eq!(noted.note.vid, 17);
    assert_eq!(noted.note.vhash, 23);
    assert!(matches!(noted.note.typ, il::ast::TypKind::BoolT));

    assert_eq!(
        [il::ast::NumOp::DecOp, il::ast::NumOp::HexOp].map(|op| match op {
            il::ast::NumOp::DecOp => "DecOp",
            il::ast::NumOp::HexOp => "HexOp",
        }),
        ["DecOp", "HexOp"]
    );
    assert_eq!(
        [
            il::ast::UnOp::NotOp,
            il::ast::UnOp::PlusOp,
            il::ast::UnOp::MinusOp
        ]
        .map(|op| match op {
            il::ast::UnOp::NotOp => "NotOp",
            il::ast::UnOp::PlusOp => "PlusOp",
            il::ast::UnOp::MinusOp => "MinusOp",
        }),
        ["NotOp", "PlusOp", "MinusOp"]
    );
    assert_eq!(
        [
            il::ast::BinOp::AndOp,
            il::ast::BinOp::OrOp,
            il::ast::BinOp::ImplOp,
            il::ast::BinOp::EquivOp,
            il::ast::BinOp::AddOp,
            il::ast::BinOp::SubOp,
            il::ast::BinOp::MulOp,
            il::ast::BinOp::DivOp,
            il::ast::BinOp::ModOp,
            il::ast::BinOp::PowOp,
        ]
        .map(|op| match op {
            il::ast::BinOp::AndOp => "AndOp",
            il::ast::BinOp::OrOp => "OrOp",
            il::ast::BinOp::ImplOp => "ImplOp",
            il::ast::BinOp::EquivOp => "EquivOp",
            il::ast::BinOp::AddOp => "AddOp",
            il::ast::BinOp::SubOp => "SubOp",
            il::ast::BinOp::MulOp => "MulOp",
            il::ast::BinOp::DivOp => "DivOp",
            il::ast::BinOp::ModOp => "ModOp",
            il::ast::BinOp::PowOp => "PowOp",
        }),
        [
            "AndOp", "OrOp", "ImplOp", "EquivOp", "AddOp", "SubOp", "MulOp", "DivOp", "ModOp",
            "PowOp",
        ]
    );
    assert_eq!(
        [
            il::ast::CmpOp::EqOp,
            il::ast::CmpOp::NeOp,
            il::ast::CmpOp::LtOp,
            il::ast::CmpOp::GtOp,
            il::ast::CmpOp::LeOp,
            il::ast::CmpOp::GeOp,
        ]
        .map(|op| match op {
            il::ast::CmpOp::EqOp => "EqOp",
            il::ast::CmpOp::NeOp => "NeOp",
            il::ast::CmpOp::LtOp => "LtOp",
            il::ast::CmpOp::GtOp => "GtOp",
            il::ast::CmpOp::LeOp => "LeOp",
            il::ast::CmpOp::GeOp => "GeOp",
        }),
        ["EqOp", "NeOp", "LtOp", "GtOp", "LeOp", "GeOp"]
    );
    assert_eq!(
        [
            il::ast::OpTyp::BoolT,
            il::ast::OpTyp::NatT,
            il::ast::OpTyp::IntT
        ]
        .map(|typ| match typ {
            il::ast::OpTyp::BoolT => "BoolT",
            il::ast::OpTyp::NatT => "NatT",
            il::ast::OpTyp::IntT => "IntT",
        }),
        ["BoolT", "NatT", "IntT"]
    );
}

fn il_exp_tag(exp: &il::ast::ExpKind) -> &'static str {
    match exp {
        il::ast::ExpKind::BoolE(_) => "BoolE",
        il::ast::ExpKind::NumE(_) => "NumE",
        il::ast::ExpKind::TextE(_) => "TextE",
        il::ast::ExpKind::VarE(_) => "VarE",
        il::ast::ExpKind::UnE(_, _, _) => "UnE",
        il::ast::ExpKind::BinE(_, _, _, _) => "BinE",
        il::ast::ExpKind::CmpE(_, _, _, _) => "CmpE",
        il::ast::ExpKind::UpCastE(_, _) => "UpCastE",
        il::ast::ExpKind::DownCastE(_, _) => "DownCastE",
        il::ast::ExpKind::SubE(_, _) => "SubE",
        il::ast::ExpKind::MatchE(_, _) => "MatchE",
        il::ast::ExpKind::TupleE(_) => "TupleE",
        il::ast::ExpKind::CaseE(_) => "CaseE",
        il::ast::ExpKind::StrE(_) => "StrE",
        il::ast::ExpKind::OptE(_) => "OptE",
        il::ast::ExpKind::ListE(_) => "ListE",
        il::ast::ExpKind::ConsE(_, _) => "ConsE",
        il::ast::ExpKind::CatE(_, _) => "CatE",
        il::ast::ExpKind::MemE(_, _) => "MemE",
        il::ast::ExpKind::LenE(_) => "LenE",
        il::ast::ExpKind::DotE(_, _) => "DotE",
        il::ast::ExpKind::IdxE(_, _) => "IdxE",
        il::ast::ExpKind::SliceE(_, _, _) => "SliceE",
        il::ast::ExpKind::UpdE(_, _, _) => "UpdE",
        il::ast::ExpKind::CallE(_, _, _) => "CallE",
        il::ast::ExpKind::IterE(_, _) => "IterE",
    }
}

#[test]
fn il_expressions_paths_parameters_and_premises_represent_every_variant() {
    let exp = || il_exp(il::ast::ExpKind::BoolE(false));
    let path = || il_path(il::ast::PathKind::RootP);
    let pattern = il::ast::Pattern::CaseP(mixop());
    let expressions = vec![
        il::ast::ExpKind::BoolE(true),
        il::ast::ExpKind::NumE(num::T::Nat(BigInt::from(7))),
        il::ast::ExpKind::TextE("text".into()),
        il::ast::ExpKind::VarE(il_id("x")),
        il::ast::ExpKind::UnE(il::ast::UnOp::NotOp, il::ast::OpTyp::BoolT, Box::new(exp())),
        il::ast::ExpKind::BinE(
            il::ast::BinOp::AddOp,
            il::ast::OpTyp::NatT,
            Box::new(exp()),
            Box::new(exp()),
        ),
        il::ast::ExpKind::CmpE(
            il::ast::CmpOp::EqOp,
            il::ast::OpTyp::BoolT,
            Box::new(exp()),
            Box::new(exp()),
        ),
        il::ast::ExpKind::UpCastE(il_typ(il::ast::TypKind::BoolT), Box::new(exp())),
        il::ast::ExpKind::DownCastE(il_typ(il::ast::TypKind::BoolT), Box::new(exp())),
        il::ast::ExpKind::SubE(Box::new(exp()), il_typ(il::ast::TypKind::BoolT)),
        il::ast::ExpKind::MatchE(Box::new(exp()), pattern),
        il::ast::ExpKind::TupleE(vec![exp()]),
        il::ast::ExpKind::CaseE(Box::new(il_not_exp(exp()))),
        il::ast::ExpKind::StrE(vec![(il_atom(), exp())]),
        il::ast::ExpKind::OptE(Some(Box::new(exp()))),
        il::ast::ExpKind::ListE(vec![exp()]),
        il::ast::ExpKind::ConsE(Box::new(exp()), Box::new(exp())),
        il::ast::ExpKind::CatE(Box::new(exp()), Box::new(exp())),
        il::ast::ExpKind::MemE(Box::new(exp()), Box::new(exp())),
        il::ast::ExpKind::LenE(Box::new(exp())),
        il::ast::ExpKind::DotE(Box::new(exp()), il_atom()),
        il::ast::ExpKind::IdxE(Box::new(exp()), Box::new(exp())),
        il::ast::ExpKind::SliceE(Box::new(exp()), Box::new(exp()), Box::new(exp())),
        il::ast::ExpKind::UpdE(Box::new(exp()), path(), Box::new(exp())),
        il::ast::ExpKind::CallE(
            il_id("call"),
            vec![phrase(il::ast::TypKind::BoolT)],
            vec![il_arg(il::ast::ArgKind::ExpA(exp()))],
        ),
        il::ast::ExpKind::IterE(
            Box::new(exp()),
            (
                il::ast::Iter::List,
                vec![(il_id("x"), il_typ(il::ast::TypKind::BoolT), vec![])],
            ),
        ),
    ];
    assert_eq!(
        expressions.iter().map(il_exp_tag).collect::<Vec<_>>(),
        [
            "BoolE",
            "NumE",
            "TextE",
            "VarE",
            "UnE",
            "BinE",
            "CmpE",
            "UpCastE",
            "DownCastE",
            "SubE",
            "MatchE",
            "TupleE",
            "CaseE",
            "StrE",
            "OptE",
            "ListE",
            "ConsE",
            "CatE",
            "MemE",
            "LenE",
            "DotE",
            "IdxE",
            "SliceE",
            "UpdE",
            "CallE",
            "IterE",
        ]
    );

    let patterns = [
        il::ast::Pattern::CaseP(mixop()),
        il::ast::Pattern::ListP(il::ast::ListPattern::Cons),
        il::ast::Pattern::OptP(il::ast::OptPattern::Some),
    ];
    assert_eq!(
        patterns.map(|pattern| match pattern {
            il::ast::Pattern::CaseP(_) => "CaseP",
            il::ast::Pattern::ListP(_) => "ListP",
            il::ast::Pattern::OptP(_) => "OptP",
        }),
        ["CaseP", "ListP", "OptP"]
    );
    assert_eq!(
        [
            il::ast::ListPattern::Cons,
            il::ast::ListPattern::Fixed(2),
            il::ast::ListPattern::Nil,
        ]
        .map(|pattern| match pattern {
            il::ast::ListPattern::Cons => "Cons",
            il::ast::ListPattern::Fixed(_) => "Fixed",
            il::ast::ListPattern::Nil => "Nil",
        }),
        ["Cons", "Fixed", "Nil"]
    );
    assert_eq!(
        [il::ast::OptPattern::Some, il::ast::OptPattern::None].map(|pattern| match pattern {
            il::ast::OptPattern::Some => "Some",
            il::ast::OptPattern::None => "None",
        }),
        ["Some", "None"]
    );

    let paths = [
        il::ast::PathKind::RootP,
        il::ast::PathKind::IdxP(Box::new(path()), Box::new(exp())),
        il::ast::PathKind::SliceP(Box::new(path()), Box::new(exp()), Box::new(exp())),
        il::ast::PathKind::DotP(Box::new(path()), il_atom()),
    ];
    assert_eq!(
        paths.map(|path| match path {
            il::ast::PathKind::RootP => "RootP",
            il::ast::PathKind::IdxP(_, _) => "IdxP",
            il::ast::PathKind::SliceP(_, _, _) => "SliceP",
            il::ast::PathKind::DotP(_, _) => "DotP",
        }),
        ["RootP", "IdxP", "SliceP", "DotP"]
    );

    let params = [
        il::ast::ParamKind::ExpP(il_typ(il::ast::TypKind::BoolT)),
        il::ast::ParamKind::DefP(
            il_id("def"),
            vec![phrase("T".into())],
            vec![il_param(il::ast::ParamKind::ExpP(il_typ(
                il::ast::TypKind::TextT,
            )))],
            il_typ(il::ast::TypKind::BoolT),
        ),
    ];
    assert_eq!(
        params.map(|param| match param {
            il::ast::ParamKind::ExpP(_) => "ExpP",
            il::ast::ParamKind::DefP(_, _, _, _) => "DefP",
        }),
        ["ExpP", "DefP"]
    );
    assert_eq!(
        [
            il::ast::ArgKind::ExpA(exp()),
            il::ast::ArgKind::DefA(il_id("def")),
        ]
        .map(|arg| match arg {
            il::ast::ArgKind::ExpA(_) => "ExpA",
            il::ast::ArgKind::DefA(_) => "DefA",
        }),
        ["ExpA", "DefA"]
    );

    let input_hint: InputHint = vec![0, 2];
    let premise = || il_prem(il::ast::PremKind::IfPr(exp()));
    let premises = vec![
        il::ast::PremKind::RulePr(il_id("rel"), il_not_exp(exp()), input_hint.clone()),
        il::ast::PremKind::IfPr(exp()),
        il::ast::PremKind::IfHoldPr(il_id("rel"), il_not_exp(exp())),
        il::ast::PremKind::IfNotHoldPr(il_id("rel"), il_not_exp(exp())),
        il::ast::PremKind::LetPr(exp(), exp()),
        il::ast::PremKind::IterPr(
            Box::new(premise()),
            (
                il::ast::Iter::List,
                vec![(il_id("x"), il_typ(il::ast::TypKind::BoolT), vec![])],
                vec![],
            ),
        ),
        il::ast::PremKind::DebugPr(exp()),
    ];
    assert_eq!(
        premises
            .into_iter()
            .map(|premise| match premise {
                il::ast::PremKind::RulePr(_, _, _) => "RulePr",
                il::ast::PremKind::IfPr(_) => "IfPr",
                il::ast::PremKind::IfHoldPr(_, _) => "IfHoldPr",
                il::ast::PremKind::IfNotHoldPr(_, _) => "IfNotHoldPr",
                il::ast::PremKind::LetPr(_, _) => "LetPr",
                il::ast::PremKind::IterPr(_, _) => "IterPr",
                il::ast::PremKind::DebugPr(_) => "DebugPr",
            })
            .collect::<Vec<_>>(),
        [
            "RulePr",
            "IfPr",
            "IfHoldPr",
            "IfNotHoldPr",
            "LetPr",
            "IterPr",
            "DebugPr"
        ]
    );
}

fn il_def_tag(definition: &il::ast::DefKind) -> &'static str {
    match definition {
        il::ast::DefKind::ExternTypD(_, _) => "ExternTypD",
        il::ast::DefKind::TypD(_, _, _, _) => "TypD",
        il::ast::DefKind::VarD(_, _, _) => "VarD",
        il::ast::DefKind::ExternRelD(_, _, _, _) => "ExternRelD",
        il::ast::DefKind::RelD(_, _, _, _, _, _) => "RelD",
        il::ast::DefKind::ExternDecD(_, _, _, _, _) => "ExternDecD",
        il::ast::DefKind::BuiltinDecD(_, _, _, _, _) => "BuiltinDecD",
        il::ast::DefKind::TableDecD(_, _, _, _, _) => "TableDecD",
        il::ast::DefKind::FuncDecD(_, _, _, _, _, _, _) => "FuncDecD",
    }
}

#[test]
fn il_rules_clauses_rows_and_definitions_preserve_phrase_structure() {
    let exp = || il_exp(il::ast::ExpKind::BoolE(true));
    let nottyp = || phrase(Mixfix::Arg(il_typ(il::ast::TypKind::BoolT)));
    let param = || il_param(il::ast::ParamKind::ExpP(il_typ(il::ast::TypKind::BoolT)));
    let arg = || il_arg(il::ast::ArgKind::ExpA(exp()));
    let prem = || il_prem(il::ast::PremKind::IfPr(exp()));
    let rule: il::ast::Rule = phrase((il_id("rule"), il_not_exp(exp()), vec![prem()]));
    let rulegroup: il::ast::RuleGroup = phrase((il_id("group"), vec![rule.clone()]));
    let elsegroup: il::ast::ElseGroup = phrase((il_id("else"), rule));
    let clause: il::ast::Clause = phrase((vec![arg()], exp(), vec![prem()]));
    let elseclause: il::ast::ElseClause = clause.clone();
    let row: il::ast::TableRow = phrase((vec![arg()], exp()));
    assert_eq!(rulegroup.it.1.len(), 1);
    assert_eq!(elsegroup.it.0.it, "else");
    assert_eq!(clause.it.0.len(), 1);
    assert_eq!(elseclause.it.2.len(), 1);
    assert_eq!(row.it.0.len(), 1);

    let deftyp = || phrase(il::ast::DefTypKind::PlainT(il_typ(il::ast::TypKind::BoolT)));
    let input_hint = vec![0];
    let definitions = vec![
        il::ast::DefKind::ExternTypD(il_id("extern-typ"), vec![il_hint()]),
        il::ast::DefKind::TypD(
            il_id("typ"),
            vec![phrase("T".into())],
            deftyp(),
            vec![il_hint()],
        ),
        il::ast::DefKind::VarD(
            il_id("var"),
            il_typ(il::ast::TypKind::BoolT),
            vec![il_hint()],
        ),
        il::ast::DefKind::ExternRelD(il_id("extern-rel"), nottyp(), input_hint.clone(), vec![]),
        il::ast::DefKind::RelD(
            il_id("rel"),
            nottyp(),
            input_hint,
            vec![rulegroup],
            Some(elsegroup),
            vec![il_hint()],
        ),
        il::ast::DefKind::ExternDecD(
            il_id("extern-dec"),
            vec![],
            vec![param()],
            il_typ(il::ast::TypKind::BoolT),
            vec![],
        ),
        il::ast::DefKind::BuiltinDecD(
            il_id("builtin-dec"),
            vec![],
            vec![param()],
            il_typ(il::ast::TypKind::BoolT),
            vec![],
        ),
        il::ast::DefKind::TableDecD(
            il_id("table-dec"),
            vec![param()],
            il_typ(il::ast::TypKind::BoolT),
            vec![row],
            vec![],
        ),
        il::ast::DefKind::FuncDecD(
            il_id("func-dec"),
            vec![],
            vec![param()],
            il_typ(il::ast::TypKind::BoolT),
            vec![clause],
            Some(elseclause),
            vec![il_hint()],
        ),
    ];
    assert_eq!(
        definitions.iter().map(il_def_tag).collect::<Vec<_>>(),
        [
            "ExternTypD",
            "TypD",
            "VarD",
            "ExternRelD",
            "RelD",
            "ExternDecD",
            "BuiltinDecD",
            "TableDecD",
            "FuncDecD",
        ]
    );

    let spec: il::ast::Spec = definitions.into_iter().map(phrase).collect();
    assert_eq!(spec.len(), 9);
    assert_eq!(spec[0].at, Region::for_file("spec/test.watsup"));
}

fn sl_param(kind: sl::ast::ParamKind) -> sl::ast::Param {
    phrase(kind)
}

fn sl_instr(kind: sl::ast::InstrKind) -> sl::ast::Instr {
    Info::with_note(
        kind,
        Region::for_file("spec/test.watsup"),
        sl::ast::INote { iid: 29 },
    )
}

fn sl_def_tag(definition: &sl::ast::DefKind) -> &'static str {
    match definition {
        sl::ast::DefKind::ExternTypD(_, _) => "ExternTypD",
        sl::ast::DefKind::TypD(_, _, _, _) => "TypD",
        sl::ast::DefKind::VarD(_, _, _) => "VarD",
        sl::ast::DefKind::ExternRelD(_) => "ExternRelD",
        sl::ast::DefKind::RelD(_) => "RelD",
        sl::ast::DefKind::ExternDecD(_) => "ExternDecD",
        sl::ast::DefKind::BuiltinDecD(_) => "BuiltinDecD",
        sl::ast::DefKind::TableDecD(_) => "TableDecD",
        sl::ast::DefKind::FuncDecD(_) => "FuncDecD",
    }
}

#[test]
fn sl_ast_represent_every_sl_only_variant_and_tuple_alias() {
    let exp = || il_exp(il::ast::ExpKind::BoolE(true));
    let typ = || il_typ(il::ast::TypKind::BoolT);
    let notexp = || il_not_exp(exp());
    let nottyp = || phrase(Mixfix::Arg(typ()));
    let param = || sl_param(sl::ast::ParamKind::ExpP(typ(), exp()));
    let signature = || (nottyp(), vec![0, 2]);
    let iterinstr = || (il::ast::Iter::List, vec![], vec![]);

    let params = [
        sl::ast::ParamKind::ExpP(typ(), exp()),
        sl::ast::ParamKind::DefP(il_id("def"), vec![phrase("T".into())], vec![param()], typ()),
    ];
    assert_eq!(
        params.map(|param| match param {
            sl::ast::ParamKind::ExpP(_, _) => "ExpP",
            sl::ast::ParamKind::DefP(_, _, _, _) => "DefP",
        }),
        ["ExpP", "DefP"]
    );

    let hold_cases = [
        sl::ast::HoldCase::BothH(vec![], vec![]),
        sl::ast::HoldCase::HoldH(vec![], false),
        sl::ast::HoldCase::NotHoldH(vec![], true),
    ];
    assert_eq!(
        hold_cases.map(|hold_case| match hold_case {
            sl::ast::HoldCase::BothH(_, _) => "BothH",
            sl::ast::HoldCase::HoldH(_, _) => "HoldH",
            sl::ast::HoldCase::NotHoldH(_, _) => "NotHoldH",
        }),
        ["BothH", "HoldH", "NotHoldH"]
    );

    let guards = [
        sl::ast::Guard::BoolG(true),
        sl::ast::Guard::CmpG(il::ast::CmpOp::EqOp, il::ast::OpTyp::BoolT, exp()),
        sl::ast::Guard::SubG(typ()),
        sl::ast::Guard::MatchG(il::ast::Pattern::CaseP(mixop())),
        sl::ast::Guard::MemG(exp()),
    ];
    assert_eq!(
        guards.map(|guard| match guard {
            sl::ast::Guard::BoolG(_) => "BoolG",
            sl::ast::Guard::CmpG(_, _, _) => "CmpG",
            sl::ast::Guard::SubG(_) => "SubG",
            sl::ast::Guard::MatchG(_) => "MatchG",
            sl::ast::Guard::MemG(_) => "MemG",
        }),
        ["BoolG", "CmpG", "SubG", "MatchG", "MemG"]
    );

    let case: sl::ast::Case = (sl::ast::Guard::BoolG(true), vec![]);
    let hold_case = || sl::ast::HoldCase::BothH(vec![], vec![]);
    let instructions = vec![
        sl::ast::InstrKind::IfI(exp(), vec![], vec![], false),
        sl::ast::InstrKind::HoldI(il_id("hold"), notexp(), vec![], hold_case()),
        sl::ast::InstrKind::CaseI(exp(), vec![case.clone()], false),
        sl::ast::InstrKind::GroupI(il_id("group"), signature(), vec![exp()], vec![]),
        sl::ast::InstrKind::LetI(exp(), exp(), vec![iterinstr()], vec![]),
        sl::ast::InstrKind::RuleI(il_id("rule"), notexp(), vec![1], vec![iterinstr()], vec![]),
        sl::ast::InstrKind::ResultI(signature(), vec![exp()]),
        sl::ast::InstrKind::ReturnI(exp()),
        sl::ast::InstrKind::DebugI(
            exp(),
            Box::new(sl_instr(sl::ast::InstrKind::ReturnI(exp()))),
        ),
    ];
    assert_eq!(
        instructions
            .iter()
            .map(|instruction| match instruction {
                sl::ast::InstrKind::IfI(_, _, _, _) => "IfI",
                sl::ast::InstrKind::HoldI(_, _, _, _) => "HoldI",
                sl::ast::InstrKind::CaseI(_, _, _) => "CaseI",
                sl::ast::InstrKind::GroupI(_, _, _, _) => "GroupI",
                sl::ast::InstrKind::LetI(_, _, _, _) => "LetI",
                sl::ast::InstrKind::RuleI(_, _, _, _, _) => "RuleI",
                sl::ast::InstrKind::ResultI(_, _) => "ResultI",
                sl::ast::InstrKind::ReturnI(_) => "ReturnI",
                sl::ast::InstrKind::DebugI(_, nested) => {
                    assert_eq!(nested.note.iid, 29);
                    assert!(matches!(nested.it, sl::ast::InstrKind::ReturnI(_)));
                    "DebugI"
                }
            })
            .collect::<Vec<_>>(),
        [
            "IfI", "HoldI", "CaseI", "GroupI", "LetI", "RuleI", "ResultI", "ReturnI", "DebugI",
        ]
    );

    let relation_signature: sl::ast::RelSignature = signature();
    let extern_relation: sl::ast::ExternRel = (
        il_id("extern-rel"),
        relation_signature.clone(),
        vec![exp()],
        vec![il_hint()],
    );
    let relation: sl::ast::Rel = (
        il_id("rel"),
        relation_signature,
        vec![exp()],
        vec![sl_instr(sl::ast::InstrKind::ReturnI(exp()))],
        Some(vec![]),
        vec![il_hint()],
    );
    let table_row: sl::ast::TableRow = (vec![exp()], exp(), vec![]);
    let extern_function: sl::ast::ExternFunc = (
        il_id("extern-func"),
        vec![],
        vec![param()],
        typ(),
        vec![il_hint()],
    );
    let builtin_function: sl::ast::BuiltinFunc = (
        il_id("builtin-func"),
        vec![],
        vec![param()],
        typ(),
        vec![il_hint()],
    );
    let table_function: sl::ast::TableFunc = (
        il_id("table-func"),
        vec![param()],
        typ(),
        vec![table_row],
        vec![il_hint()],
    );
    let defined_function: sl::ast::DefinedFunc = (
        il_id("defined-func"),
        vec![],
        vec![param()],
        typ(),
        vec![],
        Some(vec![]),
        vec![il_hint()],
    );
    assert_eq!(extern_relation.2.len(), 1);
    assert_eq!(relation.3.len(), 1);
    assert_eq!(extern_function.2.len(), 1);
    assert_eq!(builtin_function.2.len(), 1);
    assert_eq!(table_function.3.len(), 1);
    assert!(defined_function.5.is_some());

    let definitions = vec![
        sl::ast::DefKind::ExternTypD(il_id("extern-typ"), vec![il_hint()]),
        sl::ast::DefKind::TypD(
            il_id("typ"),
            vec![phrase("T".into())],
            phrase(il::ast::DefTypKind::PlainT(typ())),
            vec![il_hint()],
        ),
        sl::ast::DefKind::VarD(il_id("var"), typ(), vec![il_hint()]),
        sl::ast::DefKind::ExternRelD(extern_relation),
        sl::ast::DefKind::RelD(relation),
        sl::ast::DefKind::ExternDecD(extern_function),
        sl::ast::DefKind::BuiltinDecD(builtin_function),
        sl::ast::DefKind::TableDecD(table_function),
        sl::ast::DefKind::FuncDecD(defined_function),
    ];
    assert_eq!(
        definitions.iter().map(sl_def_tag).collect::<Vec<_>>(),
        [
            "ExternTypD",
            "TypD",
            "VarD",
            "ExternRelD",
            "RelD",
            "ExternDecD",
            "BuiltinDecD",
            "TableDecD",
            "FuncDecD",
        ]
    );

    let spec: sl::ast::Spec = definitions.into_iter().map(phrase).collect();
    assert_eq!(spec.len(), 9);
    assert_eq!(spec[0].at, Region::for_file("spec/test.watsup"));
}
