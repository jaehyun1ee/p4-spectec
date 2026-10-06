//! Tree representation of values, without arena handles
//!
//! A tree contains its child values and annotations,
//! so it can be written to JSON and read into any arena.
//! Explicit conversions intern child values before their parent,
//! preserving field and argument order.

use std::{rc::Rc, slice};

use serde::{Deserialize, Serialize};

use crate::util::json::json;

use crate::lang::{
    common::{
        Id,
        notation::atom::Atom,
        prim::num::Number,
        source::{NotePhrase, Phrase},
    },
    data::{
        arena::Arena,
        notation::{self, AtomPhrase},
        typ::TypKind,
    },
};

use super::{error::ValueError, flat};

/// An owned value with its type and source span.
pub type Value = NotePhrase<ValueKind, TypKind>;

/// A named value field.
pub type ValueField = (Phrase<Atom>, Value);

/// A value body containing its children directly.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "ValueKind")]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<ValueField>),
    Case(ValueCase),
    Tuple(Vec<Value>),
    Opt(Option<Box<Value>>),
    List(Vec<Value>),
    Func(Id),
    Extern(Rc<json>),
}

// = Arena conversion

/// Copies an arena value into a tree, including its type and source span.
pub fn from_flat(arena: &Arena, value: &flat::Value) -> Value {
    Value {
        node: ValueKind::from_flat(arena, arena.kind(value)),
        note: arena.typ(value).as_ref().clone(),
        span: arena.span(value).clone(),
    }
}

/// Interns the tree in the target arena, preserving its type and source span.
pub fn into_flat(arena: &mut Arena, value: Value) -> Result<flat::Value, ValueError> {
    let kind = value.node.into_flat(arena)?;
    arena.alloc(kind, value.note.into(), value.span)
}

impl ValueKind {
    /// Expands child handles and case shapes into trees.
    pub(super) fn from_flat(arena: &Arena, kind: &flat::ValueKind) -> Self {
        match kind {
            flat::ValueKind::Bool(value) => Self::Bool(*value),
            flat::ValueKind::Num(num) => Self::Num(num.clone()),
            flat::ValueKind::Text(text) => Self::Text(text.clone()),
            flat::ValueKind::Struct(fields) => Self::Struct(
                fields
                    .iter()
                    .map(|(atom, value)| (atom.clone(), from_flat(arena, value)))
                    .collect(),
            ),
            flat::ValueKind::Case(value_case) => {
                Self::Case(ValueCase::from_flat(arena, value_case))
            }
            flat::ValueKind::Tuple(values) => {
                Self::Tuple(values.iter().map(|value| from_flat(arena, value)).collect())
            }
            flat::ValueKind::Opt(value) => Self::Opt(
                value
                    .as_ref()
                    .map(|value| Box::new(from_flat(arena, value))),
            ),
            flat::ValueKind::List(values) => {
                Self::List(values.iter().map(|value| from_flat(arena, value)).collect())
            }
            flat::ValueKind::Func(id) => Self::Func(id.clone()),
            flat::ValueKind::Extern(json) => Self::Extern(Rc::clone(json)),
        }
    }

    /// Interns child trees in field and element order.
    pub(super) fn into_flat(self, arena: &mut Arena) -> Result<flat::ValueKind, ValueError> {
        Ok(match self {
            Self::Bool(value) => flat::ValueKind::Bool(value),
            Self::Num(num) => flat::ValueKind::Num(num),
            Self::Text(text) => flat::ValueKind::Text(text),
            Self::Struct(fields) => flat::ValueKind::Struct(
                fields
                    .into_iter()
                    .map(|(atom, value)| Ok((atom, into_flat(arena, value)?)))
                    .collect::<Result<_, ValueError>>()?,
            ),
            Self::Case(value_case) => flat::ValueKind::Case(value_case.into_flat(arena)?),
            Self::Tuple(values) => flat::ValueKind::Tuple(
                values
                    .into_iter()
                    .map(|value| into_flat(arena, value))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Opt(value) => {
                flat::ValueKind::Opt(value.map(|value| into_flat(arena, *value)).transpose()?)
            }
            Self::List(values) => flat::ValueKind::List(
                values
                    .into_iter()
                    .map(|value| into_flat(arena, value))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Func(id) => flat::ValueKind::Func(id),
            Self::Extern(json) => flat::ValueKind::Extern(json),
        })
    }
}

// = Case trees

/// A filled notation containing its argument values.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "Mixfix")]
pub enum ValueCase {
    Arg(Box<Value>),
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<ValueCase>, AtomPhrase),
    Infix(Box<ValueCase>, AtomPhrase, Box<ValueCase>),
    Seq(Vec<ValueCase>),
}

impl ValueCase {
    /// Expands a case and its arguments in notation order.
    fn from_flat(arena: &Arena, value_case: &flat::ValueCase) -> Self {
        Self::from_flat_inner(arena, *value_case.mixop(), &mut value_case.args().iter())
    }

    /// Resolves each argument as the mixop traversal reaches its position.
    fn from_flat_inner(
        arena: &Arena,
        mixop: notation::flat::Mixop,
        values: &mut slice::Iter<'_, flat::Value>,
    ) -> Self {
        match arena.arena_mixop().kind(mixop) {
            notation::flat::MixopKind::Arg => Self::Arg(Box::new(from_flat(
                arena,
                values.next().expect("a case fills every position"),
            ))),
            notation::flat::MixopKind::Atom(atom) => Self::Atom(atom.clone()),
            notation::flat::MixopKind::Brack(atom_l, mixop, atom_r) => Self::Brack(
                atom_l.clone(),
                Box::new(Self::from_flat_inner(arena, *mixop, values)),
                atom_r.clone(),
            ),
            notation::flat::MixopKind::Infix(mixop_l, atom, mixop_r) => {
                let value_case_l = Self::from_flat_inner(arena, *mixop_l, values);
                let value_case_r = Self::from_flat_inner(arena, *mixop_r, values);
                Self::Infix(Box::new(value_case_l), atom.clone(), Box::new(value_case_r))
            }
            notation::flat::MixopKind::Seq(mixops) => Self::Seq(
                mixops
                    .iter()
                    .map(|mixop| Self::from_flat_inner(arena, *mixop, values))
                    .collect(),
            ),
        }
    }

    /// Interns arguments in notation order, then their mixop.
    fn into_flat(self, arena: &mut Arena) -> Result<flat::ValueCase, ValueError> {
        // Intern argument values before the case's mixop
        let (mixop, values) = self.into_parts();
        let values = values
            .into_iter()
            .map(|value| into_flat(arena, value))
            .collect::<Result<_, _>>()?;
        let arena_mixop = arena.arena_mixop_mut();
        let mixop = notation::tree::into_flat(arena_mixop, mixop)?;

        // A filled tree supplies one value per argument position
        Ok(flat::ValueCase::new_in(arena_mixop, mixop, values)
            .expect("a tree case fills every position"))
    }

    /// Splits a filled tree into its mixop and arguments in notation order.
    fn into_parts(self) -> (notation::tree::Mixop, Vec<Value>) {
        let mut values = Vec::new();
        let mixop = self.into_parts_inner(&mut values);
        (mixop, values)
    }

    /// Moves each argument into the output as its position is visited.
    fn into_parts_inner(self, values: &mut Vec<Value>) -> notation::tree::Mixop {
        match self {
            Self::Arg(value) => {
                values.push(*value);
                notation::tree::Mixop::Arg
            }
            Self::Atom(atom) => notation::tree::Mixop::Atom(atom),
            Self::Brack(atom_l, value_case, atom_r) => notation::tree::Mixop::Brack(
                atom_l,
                Box::new(value_case.into_parts_inner(values)),
                atom_r,
            ),
            Self::Infix(value_case_l, atom, value_case_r) => {
                let mixop_l = value_case_l.into_parts_inner(values);
                let mixop_r = value_case_r.into_parts_inner(values);
                notation::tree::Mixop::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
            }
            Self::Seq(value_cases) => notation::tree::Mixop::Seq(
                value_cases
                    .into_iter()
                    .map(|value_case| value_case.into_parts_inner(values))
                    .collect(),
            ),
        }
    }
}
