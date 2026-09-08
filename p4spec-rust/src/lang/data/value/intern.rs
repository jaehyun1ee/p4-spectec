//! Borrowed keys over stored bodies; child identities bound work to one body

use std::hash::{Hash, Hasher};

use super::{SemanticId, SpanId, Value, ValueArena, ValueCase, ValueKind};
use crate::lang::common::{
    notation::{atom::Atom, mixfix::Mixfix},
    source::Phrase,
};

#[derive(Eq, PartialEq, Hash)]
enum ChildId {
    Exact(Value),
    Semantic(SemanticId),
}

pub(super) struct Key<'a> {
    pub arena: &'a ValueArena,
    pub annotations: bool,
}

impl Key<'_> {
    fn child(&self, value: &Value) -> ChildId {
        if self.annotations {
            ChildId::Exact(*value)
        } else {
            ChildId::Semantic(self.arena.semantic_id(value))
        }
    }

    fn label<'a, T>(&self, label: &'a Phrase<T, SpanId>) -> (&'a T, Option<SpanId>) {
        (&label.node, self.annotations.then_some(label.span))
    }

    pub fn equal(&self, left: &ValueKind, right: &ValueKind) -> bool {
        use ValueKind::*;
        match (left, right) {
            (Bool(left), Bool(right)) => left == right,
            (Num(left), Num(right)) => left == right,
            (Text(left), Text(right)) => left == right,
            (Struct(left), Struct(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|((label_l, value_l), (label_r, value_r))| {
                            self.label(label_l) == self.label(label_r)
                                && self.child(value_l) == self.child(value_r)
                        })
            }
            (Case(left), Case(right)) => self.equal_case(left, right),
            (Tuple(left), Tuple(right)) | (List(left), List(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| self.child(left) == self.child(right))
            }
            (Opt(left), Opt(right)) => {
                left.as_ref().map(|value| self.child(value))
                    == right.as_ref().map(|value| self.child(value))
            }
            (Func(left), Func(right)) => self.label(left) == self.label(right),
            (Extern(left), Extern(right)) => left == right,
            _ => false,
        }
    }

    pub fn hash(&self, body: &ValueKind, state: &mut impl Hasher) {
        use ValueKind::*;
        body.tag().hash(state);
        match body {
            Bool(value) => value.hash(state),
            Num(value) => value.hash(state),
            Text(value) => value.hash(state),
            Struct(fields) => {
                fields.len().hash(state);
                for (label, value) in fields {
                    self.label(label).hash(state);
                    self.child(value).hash(state);
                }
            }
            Case(value) => self.hash_case(value, state),
            Tuple(values) | List(values) => {
                values.len().hash(state);
                for value in values {
                    self.child(value).hash(state);
                }
            }
            Opt(value) => value.as_ref().map(|value| self.child(value)).hash(state),
            Func(value) => self.label(value).hash(state),
            Extern(value) => value.hash(state),
        }
    }

    // General Mixfix equality ignores atom spans; exact interning retains them
    fn equal_case(&self, left: &ValueCase, right: &ValueCase) -> bool {
        use Mixfix::*;
        match (left, right) {
            (Arg(left), Arg(right)) => self.child(left) == self.child(right),
            (Atom(left), Atom(right)) => self.label(left) == self.label(right),
            (Brack(open_l, left, close_l), Brack(open_r, right, close_r)) => {
                self.label(open_l) == self.label(open_r)
                    && self.equal_case(left, right)
                    && self.label(close_l) == self.label(close_r)
            }
            (Infix(left_l, atom_l, left_r), Infix(right_l, atom_r, right_r)) => {
                self.equal_case(left_l, right_l)
                    && self.label(atom_l) == self.label(atom_r)
                    && self.equal_case(left_r, right_r)
            }
            (Seq(left), Seq(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| self.equal_case(left, right))
            }
            _ => false,
        }
    }

    fn hash_case(&self, value: &ValueCase, state: &mut impl Hasher) {
        std::mem::discriminant(value).hash(state);
        match value {
            Mixfix::Arg(value) => self.child(value).hash(state),
            Mixfix::Atom(atom) => self.label::<Atom>(atom).hash(state),
            Mixfix::Brack(open, inner, close) => {
                self.label(open).hash(state);
                self.hash_case(inner, state);
                self.label(close).hash(state);
            }
            Mixfix::Infix(left, atom, right) => {
                self.hash_case(left, state);
                self.label(atom).hash(state);
                self.hash_case(right, state);
            }
            Mixfix::Seq(values) => {
                values.len().hash(state);
                for value in values {
                    self.hash_case(value, state);
                }
            }
        }
    }
}
