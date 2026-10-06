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

use super::{
    error::ValueError,
    flat::{self, ValueCase},
};

/// An owned value with its type and source span.
pub type Value = NotePhrase<ValueKind, TypKind>;

/// A value body containing its children directly.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "ValueKind")]
pub enum ValueKind {
    Bool(bool),
    Num(Number),
    Text(String),
    Struct(Vec<(Phrase<Atom>, Value)>),
    Case(CaseTree<Box<Value>>),
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
            flat::ValueKind::Case(value_case) => Self::Case(case_from_flat(
                arena,
                *value_case.mixop(),
                &mut value_case.args().iter(),
            )),
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
            Self::Case(tree) => flat::ValueKind::Case(into_flat_case(arena, tree)?),
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

/// Interns a case's arguments in notation order, then its mixop.
fn into_flat_case(arena: &mut Arena, tree: CaseTree<Box<Value>>) -> Result<ValueCase, ValueError> {
    let mut values = Vec::new();
    let mixop = split_case_tree(tree, &mut values);
    let values = values
        .into_iter()
        .map(|value| into_flat(arena, *value))
        .collect::<Result<_, _>>()?;
    let arena_mixop = arena.arena_mixop_mut();
    let mixop_id = arena_mixop.intern(&mixop)?;
    let value_case =
        ValueCase::new_in(arena_mixop, mixop_id, values).expect("a tree case fills every position");
    Ok(value_case)
}

// = Case trees

/// A filled notation, matching the existing case JSON topology.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "Mixfix")]
pub enum CaseTree<T> {
    Arg(T),
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<CaseTree<T>>, AtomPhrase),
    Infix(Box<CaseTree<T>>, AtomPhrase, Box<CaseTree<T>>),
    Seq(Vec<CaseTree<T>>),
}

/// Expands a runtime case directly, resolving each argument in notation order.
fn case_from_flat(
    arena: &Arena,
    mixop_id: notation::flat::Mixop,
    values: &mut slice::Iter<'_, flat::Value>,
) -> CaseTree<Box<Value>> {
    match arena.arena_mixop().kind(mixop_id) {
        notation::flat::MixopKind::Arg => CaseTree::Arg(Box::new(from_flat(
            arena,
            values.next().expect("a case fills every position"),
        ))),
        notation::flat::MixopKind::Atom(atom) => CaseTree::Atom(atom.clone()),
        notation::flat::MixopKind::Brack(atom_l, mixop_id, atom_r) => CaseTree::Brack(
            atom_l.clone(),
            Box::new(case_from_flat(arena, *mixop_id, values)),
            atom_r.clone(),
        ),
        notation::flat::MixopKind::Infix(mixop_l, atom, mixop_r) => {
            let tree_l = case_from_flat(arena, *mixop_l, values);
            let tree_r = case_from_flat(arena, *mixop_r, values);
            CaseTree::Infix(Box::new(tree_l), atom.clone(), Box::new(tree_r))
        }
        notation::flat::MixopKind::Seq(mixops) => CaseTree::Seq(
            mixops
                .iter()
                .map(|mixop| case_from_flat(arena, *mixop, values))
                .collect(),
        ),
    }
}

/// Removes arguments from a filled tree in notation order.
fn split_case_tree<T>(tree: CaseTree<T>, args: &mut Vec<T>) -> notation::tree::Mixop {
    match tree {
        CaseTree::Arg(arg) => {
            args.push(arg);
            notation::tree::Mixop::Arg
        }
        CaseTree::Atom(atom) => notation::tree::Mixop::Atom(atom),
        CaseTree::Brack(atom_l, tree, atom_r) => {
            notation::tree::Mixop::Brack(atom_l, Box::new(split_case_tree(*tree, args)), atom_r)
        }
        CaseTree::Infix(tree_l, atom, tree_r) => {
            let mixop_l = split_case_tree(*tree_l, args);
            let mixop_r = split_case_tree(*tree_r, args);
            notation::tree::Mixop::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
        }
        CaseTree::Seq(trees) => notation::tree::Mixop::Seq(
            trees
                .into_iter()
                .map(|tree| split_case_tree(tree, args))
                .collect(),
        ),
    }
}
