//! Tree representation of values, without arena handles
//!
//! A tree contains its child values and annotations,
//! so it can be written to JSON and read into any arena.
//! `Tree` boxes an option's content and owns a case's mixop;
//! `from_arena` and `into_arena` convert through `ValueNode::map` and
//! `ValueNode::try_map`, interning children before their parent.

use std::{fmt, rc::Rc, slice};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
        notation::{AtomPhrase, Mixfix, Mixop, Node},
        typ::TypKind,
    },
};

use super::{
    error::ValueError,
    flat::{Value as ArenaValue, ValueCase as ArenaValueCase, ValueKind as ArenaValueKind},
    node::{ValueNode, ValueRepr},
};

// = Representation

/// Children held as trees, each with its type and span.
#[derive(Clone, Copy, Debug)]
pub struct Tree;

impl ValueRepr for Tree {
    type Child = Box<Value>;
    type Elem = Value;
    type Mixop = Mixop;
}

// = Values

/// A value tree with its type; children are trees, not handles.
pub type Value = NotePhrase<ValueKind, TypKind>;

/// A value body whose children are trees.
pub type ValueKind = ValueNode<Tree>;

// - Debugging

// The same text a derive prints: variant names and fields, no type name
impl fmt::Debug for ValueKind {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => fmt.debug_tuple("Bool").field(value).finish(),
            Self::Num(num) => fmt.debug_tuple("Num").field(num).finish(),
            Self::Text(text) => fmt.debug_tuple("Text").field(text).finish(),
            Self::Struct(fields) => fmt.debug_tuple("Struct").field(fields).finish(),
            Self::Case(mixfix) => fmt.debug_tuple("Case").field(mixfix).finish(),
            Self::Tuple(values) => fmt.debug_tuple("Tuple").field(values).finish(),
            Self::Opt(value) => fmt.debug_tuple("Opt").field(value).finish(),
            Self::List(values) => fmt.debug_tuple("List").field(values).finish(),
            Self::Func(id) => fmt.debug_tuple("Func").field(id).finish(),
            Self::Extern(json) => fmt.debug_tuple("Extern").field(json).finish(),
        }
    }
}

// = Serialization

// Written through local enums rather than derived on the generic body:
// a derive would bound the body on its own elements, which contain it,
// and the local enums keep the variant names and shapes of the tree payload

// - Encode

impl Serialize for ValueKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename = "ValueKind")]
        enum ValueKindRef<'a> {
            Bool(&'a bool),
            Num(&'a Number),
            Text(&'a String),
            Struct(&'a [(Phrase<Atom>, Value)]),
            Case(&'a Mixfix<Mixop, Value>),
            Tuple(&'a [Value]),
            Opt(&'a Option<Box<Value>>),
            List(&'a [Value]),
            Func(&'a Id),
            Extern(&'a json),
        }

        let kind = match self {
            Self::Bool(value) => ValueKindRef::Bool(value),
            Self::Num(num) => ValueKindRef::Num(num),
            Self::Text(text) => ValueKindRef::Text(text),
            Self::Struct(fields) => ValueKindRef::Struct(fields),
            Self::Case(mixfix) => ValueKindRef::Case(mixfix),
            Self::Tuple(values) => ValueKindRef::Tuple(values),
            Self::Opt(value) => ValueKindRef::Opt(value),
            Self::List(values) => ValueKindRef::List(values),
            Self::Func(id) => ValueKindRef::Func(id),
            Self::Extern(json) => ValueKindRef::Extern(json),
        };
        kind.serialize(serializer)
    }
}

// - Decode

impl<'de> Deserialize<'de> for ValueKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "ValueKind")]
        enum ValueKindOwned {
            Bool(bool),
            Num(Number),
            Text(String),
            Struct(Vec<(Phrase<Atom>, Value)>),
            Case(Mixfix<Mixop, Value>),
            Tuple(Vec<Value>),
            Opt(Option<Box<Value>>),
            List(Vec<Value>),
            Func(Id),
            Extern(json),
        }

        Ok(match ValueKindOwned::deserialize(deserializer)? {
            ValueKindOwned::Bool(value) => Self::Bool(value),
            ValueKindOwned::Num(num) => Self::Num(num),
            ValueKindOwned::Text(text) => Self::Text(text),
            ValueKindOwned::Struct(fields) => Self::Struct(fields),
            ValueKindOwned::Case(mixfix) => Self::Case(mixfix),
            ValueKindOwned::Tuple(values) => Self::Tuple(values),
            ValueKindOwned::Opt(value) => Self::Opt(value),
            ValueKindOwned::List(values) => Self::List(values),
            ValueKindOwned::Func(id) => Self::Func(id),
            ValueKindOwned::Extern(json) => Self::Extern(Rc::new(json)),
        })
    }
}

// = Arena conversion

/// Copies an arena value into a tree, including its type and source span.
pub fn from_arena(arena: &Arena, value: &ArenaValue) -> Value {
    Value {
        node: ValueKind::from_arena(arena, arena.kind(value)),
        note: arena.typ(value).as_ref().clone(),
        span: arena.span(value).clone(),
    }
}

/// Interns the tree in the target arena, preserving its type and source span.
pub fn into_arena(arena: &mut Arena, value: Value) -> Result<ArenaValue, ValueError> {
    let kind = value.node.into_arena(arena)?;
    arena.alloc(kind, value.note.into(), value.span)
}

impl ValueKind {
    /// Expands child handles and case shapes into trees.
    pub(super) fn from_arena(arena: &Arena, kind: &ArenaValueKind) -> Self {
        kind.map(
            |value| from_arena(arena, value),
            |value_case| {
                value_case
                    .to_tree(arena.arena_shape())
                    .map_into(|value| from_arena(arena, &value))
            },
        )
    }

    /// Interns child trees in field and element order.
    pub(super) fn into_arena(self, arena: &mut Arena) -> Result<ArenaValueKind, ValueError> {
        self.try_map(arena, into_arena, |arena, value| into_arena(arena, *value), into_arena_case)
    }
}

/// Interns a case's arguments in notation order, then its mixop.
fn into_arena_case(
    arena: &mut Arena,
    mixfix: Mixfix<Mixop, Value>,
) -> Result<ArenaValueCase, ValueError> {
    let (mixop, values) = mixfix.into_parts();
    let values = values
        .into_iter()
        .map(|value| into_arena(arena, value))
        .collect::<Result<_, _>>()?;
    let arena_shape = arena.arena_shape_mut();
    let shape = arena_shape.intern(&mixop)?;
    let value_case = ArenaValueCase::new_in(arena_shape, shape, values)
        .expect("a tree case fills every position");
    Ok(value_case)
}

// = Case trees

// A case is written as its filled notation tree,
// with the variant names and shapes of the former `Mixfix`

/// A case written as its filled notation tree.
#[derive(Serialize)]
#[serde(rename = "Mixfix")]
enum CaseTree<'a, T> {
    Arg(&'a T),
    Atom(&'a AtomPhrase),
    Brack(&'a AtomPhrase, Box<CaseTree<'a, T>>, &'a AtomPhrase),
    Infix(Box<CaseTree<'a, T>>, &'a AtomPhrase, Box<CaseTree<'a, T>>),
    Seq(Vec<CaseTree<'a, T>>),
}

/// A case read as its filled notation tree.
#[derive(Deserialize)]
#[serde(rename = "Mixfix")]
enum CaseTreeOwned<T> {
    Arg(T),
    Atom(AtomPhrase),
    Brack(AtomPhrase, Box<CaseTreeOwned<T>>, AtomPhrase),
    Infix(Box<CaseTreeOwned<T>>, AtomPhrase, Box<CaseTreeOwned<T>>),
    Seq(Vec<CaseTreeOwned<T>>),
}

/// Fills a mixop with arguments in notation order, borrowing its atoms.
fn case_tree<'a, T>(mixop: &'a Mixop, args: &mut slice::Iter<'a, T>) -> CaseTree<'a, T> {
    match mixop {
        Node::Arg => CaseTree::Arg(args.next().expect("a case fills every position")),
        Node::Atom(atom) => CaseTree::Atom(atom),
        Node::Brack(atom_l, mixop_inner, atom_r) => {
            CaseTree::Brack(atom_l, Box::new(case_tree(mixop_inner, args)), atom_r)
        }
        Node::Infix(mixop_l, atom, mixop_r) => {
            let tree_l = case_tree(mixop_l, args);
            let tree_r = case_tree(mixop_r, args);
            CaseTree::Infix(Box::new(tree_l), atom, Box::new(tree_r))
        }
        Node::Seq(mixops) => {
            CaseTree::Seq(mixops.iter().map(|mixop| case_tree(mixop, args)).collect())
        }
    }
}

/// Splits a read tree into its mixop, moving arguments out in notation order.
fn split_case_tree<T>(tree: CaseTreeOwned<T>, args: &mut Vec<T>) -> Mixop {
    match tree {
        CaseTreeOwned::Arg(arg) => {
            args.push(arg);
            Node::Arg
        }
        CaseTreeOwned::Atom(atom) => Node::Atom(atom),
        CaseTreeOwned::Brack(atom_l, tree, atom_r) => {
            Node::Brack(atom_l, Box::new(split_case_tree(*tree, args)), atom_r)
        }
        CaseTreeOwned::Infix(tree_l, atom, tree_r) => {
            let mixop_l = split_case_tree(*tree_l, args);
            let mixop_r = split_case_tree(*tree_r, args);
            Node::Infix(Box::new(mixop_l), atom, Box::new(mixop_r))
        }
        CaseTreeOwned::Seq(trees) => Node::Seq(
            trees
                .into_iter()
                .map(|tree| split_case_tree(tree, args))
                .collect(),
        ),
    }
}

impl<T: Serialize> Serialize for Mixfix<Mixop, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        case_tree(self.mixop(), &mut self.args().iter()).serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Mixfix<Mixop, T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut args = Vec::new();
        let mixop = split_case_tree(CaseTreeOwned::deserialize(deserializer)?, &mut args);
        Ok(Mixfix::new(mixop, args).expect("a tree case fills every position"))
    }
}
