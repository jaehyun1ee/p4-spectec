//! Values shared by the intermediate language representations

use std::rc::Rc;

use crate::{
    lang::{
        common::{
            Id,
            notation::{atom, mixfix::Mixfix},
            source::{NotePhrase, Phrase},
        },
        data::typ::TypKind,
        xl::num::Number,
    },
    yojson::ExternalData,
};

pub type Value = NotePhrase<ValueKind, TypKind>;

#[derive(Clone, Debug)]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<ValueField>),
    Case(ValueCase),
    Tuple(Vec<Rc<Value>>),
    Opt(Option<Rc<Value>>),
    List(Vec<Rc<Value>>),
    Func(Id),
    Extern(ExternalData),
}

pub type ValueField = (Phrase<atom::Atom>, Rc<Value>);
pub type ValueCase = Mixfix<Rc<Value>>;
